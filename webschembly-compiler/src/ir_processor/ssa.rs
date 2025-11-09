use crate::ir_processor::cfg_analyzer::DomTreeNode;
use crate::ir_processor::cfg_analyzer::{
    build_dom_tree, calc_dominance_frontiers_from_tree, calc_doms, calc_predecessors,
    calculate_rpo, has_critical_edges,
};
use rustc_hash::{FxHashMap, FxHashSet};
use vec_map::{HasId, VecMap};
use webschembly_compiler_ir::*;

// 前提条件: クリティカルエッジが存在しない
pub fn remove_phi(func: &mut Func) {
    debug_assert_no_critical_edge(func);
    debug_assert_phi_rules(func);
    let bb_ids = func.bbs.keys().collect::<Vec<_>>();

    for bb_id in bb_ids {
        remove_phi_in_bb(func, bb_id);
    }
}

fn debug_assert_no_critical_edge(func: &Func) {
    if cfg!(debug_assertions) && has_critical_edges(&func.bbs) {
        panic!("Function has critical edges");
    }
}

fn debug_assert_phi_rules(func: &Func) {
    if cfg!(debug_assertions) {
        assert_phi_rules(func);
    }
}

/*
phiのルール:
- 基本ブロックの先頭にまとめて置く(NOPが間に入ってもよい)
- fromで指定できるBBは必ず先行ブロックである
- non exhaustive: falseでなければならない
*/
fn assert_phi_rules(func: &Func) {
    let predecessors = calc_predecessors(&func.bbs);

    for bb in func.bbs.values() {
        let mut phi_area = true;

        for expr in bb.instrs.iter() {
            if phi_area {
                match &expr.kind {
                    InstrKind::Phi(incomings, non_exhaustive) => {
                        if *non_exhaustive {
                            panic!("Phi instruction must be exhaustive");
                        }
                        for incoming in incomings {
                            if !predecessors[&bb.id].contains(&incoming.bb) {
                                panic!(
                                    "Phi instruction incoming bb {:?} is not a predecessor of bb {:?}",
                                    incoming.bb, bb.id
                                );
                            }
                        }
                    }
                    InstrKind::Nop => {
                        // do nothing
                    }
                    _ => {
                        phi_area = false;
                    }
                }
            } else if let InstrKind::Phi(_, _) = expr.kind {
                panic!("phi instruction must be at the beginning of a basic block");
            }
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
        if let InstrKind::Phi(incomings, _) = &instr.kind
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

        bb.insert_instrs_before_terminator(sequential_copies.iter().map(|copy| Instr {
            local: Some(copy.dest),
            kind: InstrKind::Move(copy.src),
        }));
    }

    // 対象ブロックのPHI命令を削除
    for instr in &mut func.bbs[bb_id].instrs {
        if let InstrKind::Phi(_, _) = &instr.kind {
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
                if let InstrKind::Phi(_, _) | InstrKind::Nop = expr.kind {
                    // do nothing
                } else {
                    phi_area = false;
                }
            } else if let InstrKind::Phi(_, _) = expr.kind {
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
            let instr = &bbs[def.bb_id].instrs[def.expr_idx];
            debug_assert_eq!(instr.local, Some(local));
            Some(&instr.kind)
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
                InstrKind::Phi(incomings, non_exhaustive)
                    if incomings.len() == 1 && !*non_exhaustive =>
                {
                    local = incomings[0].local;
                }
                _ => {
                    return Some(expr);
                }
            }
        }
        None
    }
}

pub fn build_ssa(func: &mut Func) {
    // ステップ1: 支配木と支配辺境の計算に必要な情報を準備
    let predecessors = calc_predecessors(&func.bbs);
    let rpo = calculate_rpo(&func.bbs, func.bb_entry);
    let doms = calc_doms(&func.bbs, &rpo, func.bb_entry, &predecessors);
    let dom_tree = build_dom_tree(&func.bbs, &rpo, func.bb_entry, &doms);
    let dominance_frontiers =
        calc_dominance_frontiers_from_tree(&func.bbs, &dom_tree, &predecessors);

    // ステップ2: 各変数が定義されているブロックを収集
    let mut def_blocks: FxHashMap<LocalId, Vec<BasicBlockId>> = FxHashMap::default();
    for (bb_id, bb) in func.bbs.iter() {
        for instr in &bb.instrs {
            if let Some(local) = instr.local {
                def_blocks.entry(local).or_default().push(bb_id);
            }
        }
    }

    // ステップ3: Phi命令の挿入 (Iterated Dominance Frontier)
    let mut has_phi: FxHashSet<(BasicBlockId, LocalId)> = FxHashSet::default();
    for (&var, blocks) in def_blocks.iter() {
        let mut worklist: Vec<BasicBlockId> = blocks.clone();
        let mut visited: FxHashSet<BasicBlockId> = FxHashSet::default();

        while let Some(bb_id) = worklist.pop() {
            for &df in dominance_frontiers
                .get(&bb_id)
                .unwrap_or(&FxHashSet::default())
            {
                if has_phi.insert((df, var)) {
                    // insert phi at beginning of df block
                    let phi_instr = Instr {
                        local: Some(var),
                        kind: InstrKind::Phi(Vec::new(), false), // 引数は後で充填
                    };
                    let bb = &mut func.bbs[df];
                    // insert at first position after any existing phi/nop
                    let mut insert_idx = 0usize;
                    for (i, instr) in bb.instrs.iter().enumerate() {
                        match &instr.kind {
                            InstrKind::Phi(_, _) | InstrKind::Nop => insert_idx = i + 1,
                            _ => break,
                        }
                    }
                    bb.instrs.insert(insert_idx, phi_instr);

                    if visited.insert(df) {
                        // newly created phi is considered a def site for this var
                        worklist.push(df);
                    }
                }
            }
        }
    }

    // ステップ4: 変数のリネーム (支配木を使用)
    // 各「元の」変数ID用のスタックを準備
    let mut stacks: FxHashMap<LocalId, Vec<LocalId>> = FxHashMap::default();
    for local in func.locals.keys() {
        stacks.insert(local, Vec::new());
    }

    // 関数の引数をスタックの初期値としてプッシュ
    for &arg in &func.args {
        stacks.get_mut(&arg).unwrap().push(arg);
    }

    // 新しいLocalIdがどの「元の」LocalIdに対応するかを追跡
    let mut original_of: FxHashMap<LocalId, LocalId> = FxHashMap::default();

    // 再帰的なリネーム関数
    fn rename_block(
        node: &DomTreeNode,
        func: &mut Func,
        stacks: &mut FxHashMap<LocalId, Vec<LocalId>>,
        original_of: &mut FxHashMap<LocalId, LocalId>,
    ) {
        let bb_id = node.id;

        // このブロックで作成された定義を追跡し、関数の最後でポップするため
        let mut local_defs: Vec<LocalId> = Vec::new();

        // Phase A: PHI命令を処理 (新しい定義を作成)
        let mut phi_updates: Vec<(usize, LocalId)> = Vec::new();
        for (idx, instr) in func.bbs[bb_id].instrs.iter().enumerate() {
            if let InstrKind::Phi(_, _) = instr.kind {
                if let Some(orig) = instr.local {
                    let typ = func.locals[orig].typ;
                    let new_local = func.locals.push_with(|id| Local { id, typ });
                    original_of.insert(new_local, orig);
                    stacks.get_mut(&orig).unwrap().push(new_local);
                    local_defs.push(orig); // 後でポップするために元の変数を記録
                    phi_updates.push((idx, new_local));
                }
            } else {
                // PHIはブロックの先頭にあるはず
                break;
            }
        }

        // PHI命令の定義側(local)を新しいIDに更新
        for (idx, new_local) in phi_updates {
            func.bbs[bb_id].instrs[idx].local = Some(new_local);
        }

        // Phase B: 通常の命令を処理 (使用側をリネームし、新しい定義を作成)
        let num_instrs = func.bbs[bb_id].instrs.len();
        for idx in 0..num_instrs {
            let instr = &func.bbs[bb_id].instrs[idx];
            if let InstrKind::Phi(_, _) = instr.kind {
                continue; // PHIはPhase Aで処理済み
            }

            // この命令の「使用(use)」をリネーム
            // (借用チェッカのため、一旦変更内容を収集)
            let mut use_replacements: Vec<(usize, LocalId)> = Vec::new();
            for (i, (local_ref, flag)) in instr.local_usages().enumerate() {
                if let LocalFlag::Used(_) = flag {
                    // スタックのトップにある最新のローカルIDで置換
                    if let Some(&top) = stacks.get(local_ref).and_then(|s| s.last()) {
                        use_replacements.push((i, top));
                    }
                    // else: スタックが空 (未定義変数の使用)。
                    // 本来はエラーだが、ここでは何もしない (IR検証で検出)
                }
            }

            // 収集した変更を適用
            let instr_mut = &mut func.bbs[bb_id].instrs[idx];
            let usage_iter = instr_mut.local_usages_mut();
            for (i, (local_ref, _flag)) in usage_iter.enumerate() {
                if let Some(&(_idx, new_val)) = use_replacements.iter().find(|(idx, _)| *idx == i) {
                    *local_ref = new_val;
                }
            }

            // この命令の「定義(def)」をリネーム (instr.local)
            // (借用チェッカのため、instrのイテレートと分離)
            let mut def_replacement: Option<LocalId> = None;
            if let Some(orig) = func.bbs[bb_id].instrs[idx].local {
                // PHI以外の命令は `local_usages` で "Defined" を返さない設計と仮定
                // (もし `local_usages` で "Defined" を扱うなら、ここのロジックは
                //  上の `local_usages` ループ内に移動する必要がある)

                // ここでは、`instr.local` が定義スロットであると仮定する
                let typ = func.locals[orig].typ;
                let new_local = func.locals.push_with(|id| Local { id, typ });
                original_of.insert(new_local, orig);
                stacks.get_mut(&orig).unwrap().push(new_local);
                local_defs.push(orig); // 後でポップするために元の変数を記録
                def_replacement = Some(new_local);
            }

            if let Some(new_def) = def_replacement {
                func.bbs[bb_id].instrs[idx].local = Some(new_def);
            }
        }

        // Phase C: CFGの後続ブロック (Successors) のPHI引数を充填
        // (このブロックのリネームがすべて完了し、スタックが最新の状態で実行)
        //

        // 借用チェッカを通過するため、後続IDを先に収集
        // (func.bbs[bb_id]のイミュータブルな参照とfunc.bbs[succ_id]のミュータブルな参照が衝突するため)
        let successors: Vec<BasicBlockId> = func.bbs[bb_id].terminator().successors().collect();

        for &succ_id in &successors {
            let succ_bb = &mut func.bbs[succ_id];
            for instr in succ_bb.instrs.iter_mut() {
                if let InstrKind::Phi(incomings, _) = &mut instr.kind {
                    if let Some(dest_local) = instr.local {
                        // このPHIが対応する「元の」変数を探す
                        if let Some(&orig) = original_of.get(&dest_local) {
                            // このブロック(bb_id)を抜ける時点での「元の」変数の
                            // 最新の値 (スタックのトップ) を取得
                            if let Some(&current_val) = stacks.get(&orig).and_then(|s| s.last()) {
                                // (bb_id から来た場合, 値は current_val) を追加
                                incomings.push(PhiIncomingValue {
                                    bb: bb_id, // bb_id = この現在のブロック
                                    local: current_val,
                                });
                            }
                            // else: スタックが空 (未定義パス)。
                            // これは通常、エントリーブロックに到達するパスで
                            // 引数でも定義されていなかった場合など。
                        }
                    }
                } else {
                    // PHI命令はブロックの先頭に固まっているはず
                    break;
                }
            }
        }
        for child in &node.children {
            rename_block(child, func, stacks, original_of);
        }

        for orig in local_defs.into_iter().rev() {
            stacks.get_mut(&orig).unwrap().pop();
        }
    }

    rename_block(&dom_tree, func, &mut stacks, &mut original_of);
}
