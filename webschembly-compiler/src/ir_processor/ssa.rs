use crate::{
    VecMap,
    ir::{BasicBlock, BasicBlockId, Expr, ExprAssign, Func, Local, LocalId},
};
use rustc_hash::{FxHashMap, FxHashSet};

// 前提条件: クリティカルエッジが存在しない
pub fn remove_phi(func: &mut Func) {
    let bb_ids = func.bbs.keys().collect::<Vec<_>>();

    for bb_id in bb_ids {
        remove_phi_in_bb(func, bb_id);
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
    for expr_assign in &func.bbs[bb_id].exprs {
        if let Expr::Phi(incomings) = &expr_assign.expr
            && let Some(result) = expr_assign.local
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
            bb.exprs.push(ExprAssign {
                local: Some(copy.dest),
                expr: Expr::Move(copy.src),
            });
        }
    }

    // 対象ブロックのPHI命令を削除
    for expr_assign in &mut func.bbs[bb_id].exprs {
        if let Expr::Phi(_) = &expr_assign.expr {
            expr_assign.expr = Expr::Nop;
            expr_assign.local = None;
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
        for expr in bb.exprs.iter() {
            if let Some(local_id) = expr.local {
                if assigned[local_id] {
                    panic!("local {:?} is assigned more than once", local_id);
                }
                assigned[local_id] = true;
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
    for (i, expr_assign) in bb.exprs.iter().enumerate() {
        if let Some(local) = expr_assign.local {
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
    pub bb_id: BasicBlockId,
    pub expr_idx: usize,
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
                bb_id: bb.id,
                expr_idx: idx,
            };
            // 既に存在する場合は同じ定義である
            debug_assert!(self.defs.get(local).map(|&x| x == def).unwrap_or(true));
            self.defs.insert(local, def);
        }
    }

    pub fn get_def<'a>(
        &self,
        bbs: &'a VecMap<BasicBlockId, BasicBlock>,
        local: LocalId,
    ) -> Option<&'a Expr> {
        if let Some(def) = self.defs.get(local) {
            Some(&bbs[def.bb_id].exprs[def.expr_idx].expr)
        } else {
            None
        }
    }

    pub fn get_non_move_def<'a>(
        &self,
        bbs: &'a VecMap<BasicBlockId, BasicBlock>,
        mut local: LocalId,
    ) -> Option<&'a Expr> {
        while let Some(expr) = self.get_def(bbs, local) {
            match expr {
                Expr::Move(value) => {
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
