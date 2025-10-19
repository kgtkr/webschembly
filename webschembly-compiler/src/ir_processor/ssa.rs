use rustc_hash::{FxHashMap, FxHashSet};
use vec_map::{HasId, VecMap};
use webschembly_compiler_ir::*;

use crate::ir_processor::cfg_analyzer::has_critical_edges;

// 前提条件: クリティカルエッジが存在しない
pub fn remove_phi(func: &mut Func) {
    debug_assert_no_critical_edge(func);
    let bb_ids = func.bbs.keys().collect::<Vec<_>>();

    for bb_id in bb_ids {
        remove_phi_in_bb(func, bb_id);
    }
}

fn debug_assert_no_critical_edge(func: &Func) {
    if cfg!(debug_assertions) {
        if has_critical_edges(&func.bbs) {
            panic!("Function has critical edges");
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Copy {
    pub dest: LocalId,
    pub src: LocalId,
}

fn remove_phi_in_bb(func: &mut Func, bb_id: BasicBlockId) {
    // 先行ブロックごとのコピーリスト(並列コピーセマンティクス)
    let mut pending_copies: FxHashMap<BasicBlockId, Vec<Copy>> = FxHashMap::default();

    // 先行ブロックごとの並列コピーリストを収集
    for instr in &func.bbs[bb_id].instrs {
        if let InstrKind::Phi(incomings) = &instr.kind
            && let Some(result) = instr.local
        {
            for incoming in incomings {
                pending_copies.entry(incoming.bb).or_default().push(Copy {
                    dest: result,
                    src: incoming.local,
                });
            }
        }
    }

    // 並列コピーを逐次コピーに変換して、先行ブロックの末尾に挿入
    for (block_id, parallel_copies) in pending_copies {
        let sequential_copies = sequentialize_parallel_copies(func, parallel_copies);
        let bb = &mut func.bbs[block_id];

        for copy in sequential_copies {
            bb.instrs.push(Instr {
                local: Some(copy.dest),
                kind: InstrKind::Move(copy.src),
            });
        }
    }

    // 対象ブロックのPHI命令を削除
    for instr in &mut func.bbs[bb_id].instrs {
        if let InstrKind::Phi(_) = &instr.kind {
            instr.kind = InstrKind::Nop;
            instr.local = None;
        }
    }
}

// 並列コピーを逐次コピーに変換
fn sequentialize_parallel_copies(func: &mut Func, parallel_copies: Vec<Copy>) -> Vec<Copy> {
    let mut todo = parallel_copies;
    let mut result = Vec::new();
    while !todo.is_empty() {
        let sources_in_todo = todo.iter().map(|c| c.src).collect::<FxHashSet<_>>();
        let mut ready_copies = Vec::new();
        let mut not_ready_copies = Vec::new();
        for copy in todo.drain(..) {
            if sources_in_todo.contains(&copy.dest) {
                not_ready_copies.push(copy);
            } else {
                ready_copies.push(copy);
            }
        }
        if !ready_copies.is_empty() {
            result.extend(&ready_copies);
            todo = not_ready_copies;
        } else {
            let cycle_copy = not_ready_copies.pop().unwrap();
            debug_assert_eq!(
                func.locals[cycle_copy.src].typ,
                func.locals[cycle_copy.dest].typ
            );
            let typ = func.locals[cycle_copy.src].typ;
            let temp = func.locals.push_with(|id| Local { id, typ });
            result.push(Copy {
                dest: temp,
                src: cycle_copy.src,
            });
            not_ready_copies.push(Copy {
                dest: cycle_copy.dest,
                src: temp,
            });
            todo = not_ready_copies;
        }
    }
    result
}

fn assert_ssa(func: &Func) {
    let mut assigned = VecMap::new();
    for local_id in func.locals.keys() {
        assigned.insert(local_id, false);
    }
    for &local_id in func.args.iter() {
        assigned[local_id] = true;
    }
    for bb in func.bbs.values() {
        let mut phi_area = true;

        for expr in bb.instrs.iter() {
            if let Some(local_id) = expr.local {
                if assigned[local_id] {
                    panic!("local {:?} is assigned more than once", local_id);
                }
                assigned[local_id] = true;
            }

            if phi_area {
                if let InstrKind::Phi(_) | InstrKind::Nop = expr.kind {
                    // do nothing
                } else {
                    phi_area = false;
                }
            } else if let InstrKind::Phi(_) = expr.kind {
                panic!("phi instruction must be at the beginning of a basic block");
            }
        }
    }
}

pub fn debug_assert_ssa(func: &Func) {
    if cfg!(debug_assertions) {
        assert_ssa(func);
    }
}

// TODO: test
pub fn collect_defs(bb: &BasicBlock) -> VecMap<LocalId, usize> {
    let mut defs = VecMap::new();
    for (i, instr) in bb.instrs.iter().enumerate() {
        if let Some(local) = instr.local {
            debug_assert!(defs.get(local).is_none());
            defs.insert(local, i);
        }
    }
    defs
}

#[derive(Debug, Clone)]
pub struct DefUseChain {
    defs: VecMap<LocalId, Def>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Def {
    pub local: LocalId,
    pub bb_id: BasicBlockId,
    pub expr_idx: usize,
}

impl HasId for Def {
    type Id = LocalId;
    fn id(&self) -> Self::Id {
        self.local
    }
}

impl Default for DefUseChain {
    fn default() -> Self {
        Self::new()
    }
}

impl DefUseChain {
    pub fn new() -> Self {
        Self {
            defs: VecMap::new(),
        }
    }

    pub fn remove(&mut self, local: LocalId) {
        self.defs.remove(local);
    }

    pub fn from_bbs(bbs: &VecMap<BasicBlockId, BasicBlock>) -> Self {
        let mut chain = Self::new();
        for bb in bbs.values() {
            chain.add_bb(bb);
        }
        chain
    }

    pub fn add_bb(&mut self, bb: &BasicBlock) {
        let defs = collect_defs(bb);
        for (local, idx) in defs {
            let def = Def {
                local,
                bb_id: bb.id,
                expr_idx: idx,
            };
            // 既に存在する場合は同じ定義である
            debug_assert!(self.defs.get(local).map(|&x| x == def).unwrap_or(true));
            self.defs.insert_node(def);
        }
    }

    pub fn get_def(&self, local: LocalId) -> Option<Def> {
        self.defs.get(local).copied()
    }

    pub fn get_def_expr<'a>(
        &self,
        bbs: &'a VecMap<BasicBlockId, BasicBlock>,
        local: LocalId,
    ) -> Option<&'a InstrKind> {
        if let Some(def) = self.defs.get(local) {
            Some(&bbs[def.bb_id].instrs[def.expr_idx].kind)
        } else {
            None
        }
    }

    pub fn get_def_non_move_expr<'a>(
        &self,
        bbs: &'a VecMap<BasicBlockId, BasicBlock>,
        mut local: LocalId,
    ) -> Option<&'a InstrKind> {
        while let Some(expr) = self.get_def_expr(bbs, local) {
            match expr {
                InstrKind::Move(value) => {
                    local = *value;
                }
                _ => {
                    return Some(expr);
                }
            }
        }
        None
    }
}
