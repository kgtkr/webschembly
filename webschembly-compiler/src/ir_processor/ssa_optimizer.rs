use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir_processor::{
    cfg_analyzer::{DomTreeNode, build_dom_tree, calc_doms, calc_predecessors, calculate_rpo},
    ssa::{DefUseChain, debug_assert_ssa},
};
use vec_map::VecMap;
use webschembly_compiler_ir::*;

// 制限事項: ループなどで循環参照がある純粋な命令同士は削除できない
pub fn dead_code_elimination(func: &mut Func, def_use: &mut DefUseChain) {
    let mut use_counts = VecMap::new();
    for local_id in func.locals.keys() {
        use_counts.insert(local_id, 0);
    }
    for bb in func.bbs.values() {
        for (&local, flag) in bb.local_usages() {
            if let LocalFlag::Used(_) = flag {
                use_counts[local] += 1;
            }
        }
    }

    let mut worklist = Vec::new();
    for (local_id, &count) in use_counts.iter() {
        if count == 0 {
            let def = def_use.get_def(local_id);
            if let Some(def) = def {
                worklist.push(def);
            }
        }
    }
    while let Some(def) = worklist.pop() {
        let instr = &mut func.bbs[def.bb_id].instrs[def.expr_idx];
        instr.local = None;
        def_use.remove(def.local);

        if instr.kind.purelity().can_dce() {
            for (&operand, _) in instr.kind.local_usages() {
                let count = &mut use_counts[operand];
                *count -= 1;
                if *count == 0
                    && let Some(def) = def_use.get_def(operand)
                {
                    worklist.push(def);
                }
            }
            instr.kind = InstrKind::Nop;
        }
    }
}

pub fn copy_propagation(func: &mut Func, rpo: &FxHashMap<BasicBlockId, usize>) {
    let mut rpo_nodes = func.bbs.keys().collect::<Vec<_>>();
    rpo_nodes.sort_by_key(|id| rpo.get(id).unwrap());

    let mut copies = FxHashMap::default();

    for bb_id in &rpo_nodes {
        let bb = &mut func.bbs[*bb_id];

        for instr in &bb.instrs {
            match *instr {
                Instr {
                    local: Some(dest),
                    kind: InstrKind::Move(src),
                } => {
                    let src = copies.get(&src).copied().unwrap_or(src);
                    copies.insert(dest, src);
                }
                Instr {
                    local: Some(dest),
                    kind: InstrKind::Phi(ref incomings),
                } => {
                    let mut all_same = true;
                    let mut first = None;
                    for incoming in incomings {
                        let src = copies
                            .get(&incoming.local)
                            .copied()
                            .unwrap_or(incoming.local);
                        if let Some(first) = first {
                            if first != src {
                                all_same = false;
                                break;
                            }
                        } else {
                            first = Some(src);
                        }
                    }
                    if all_same && let Some(src) = first {
                        copies.insert(dest, src);
                    }
                }
                _ => {}
            }
        }

        for (local, flag) in bb.local_usages_mut() {
            if let LocalFlag::Used(_) = flag
                && let Some(&src) = copies.get(local)
            {
                *local = src;
            }
        }
    }
}

pub fn common_subexpression_elimination(func: &mut Func, dom_tree: &DomTreeNode) {
    let mut expr_map = FxHashMap::default();
    common_subexpression_elimination_rec(func, dom_tree, &mut expr_map);
}

fn common_subexpression_elimination_rec(
    func: &mut Func,
    dom_tree: &DomTreeNode,
    expr_map: &mut FxHashMap<InstrKind, LocalId>,
) {
    let bb = &mut func.bbs[dom_tree.id];
    for instr in &mut bb.instrs {
        if instr.local.is_none() {
            continue;
        }
        if !instr.kind.purelity().can_cse() {
            continue;
        }
        if let Some(&existing) = expr_map.get(&instr.kind) {
            instr.kind = InstrKind::Move(existing);
        } else if instr.local.is_some() {
            expr_map.insert(instr.kind.clone(), instr.local.unwrap());
        }
    }

    for child in &dom_tree.children {
        let mut expr_map = expr_map.clone();
        common_subexpression_elimination_rec(func, child, &mut expr_map);
    }
}

pub fn eliminate_redundant_obj(func: &mut Func, def_use: &DefUseChain) {
    // rpo順に処理したほうがいいかも
    for local in func.locals.keys() {
        let Some(def) = def_use.get_def(local) else {
            continue;
        };
        /*
        typ1 == typ2 でないものが到達不能コードとして現れる可能性がある
        例:

        l1 = to_obj<int>(..)
        l2 = is<string>(l1)
        if l2 {
            l3 = from_obj<string>(l1) // ここは到達不能であるので無視してよい
        }
        */
        match func.bbs[def.bb_id].instrs[def.expr_idx].kind {
            InstrKind::ToObj(typ1, src) => {
                if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                    && let InstrKind::FromObj(typ2, obj_src) = *src_expr
                    && typ1 == typ2
                {
                    func.bbs[def.bb_id].instrs[def.expr_idx].kind = InstrKind::Move(obj_src);
                }
            }
            InstrKind::FromObj(typ1, src) => {
                if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                    && let InstrKind::ToObj(typ2, obj_src) = *src_expr
                    && typ1 == typ2
                {
                    func.bbs[def.bb_id].instrs[def.expr_idx].kind = InstrKind::Move(obj_src);
                }
            }
            _ => {}
        }
    }
}

pub fn constant_folding(
    func: &mut Func,
    rpo: &FxHashMap<BasicBlockId, usize>,
    def_use: &DefUseChain,
    doms: &FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>>,
) {
    // ClosureSetEnvの収集
    // ClosureSetEnvは各Closure/indexに対して一度のみしか実行されないことが保証されているため定数畳み込み可能
    let mut closure_set_envs = FxHashMap::default();
    for bb_id in func.bbs.keys() {
        for (expr_idx, instr) in func.bbs[bb_id].instrs.iter().enumerate() {
            if let InstrKind::ClosureSetEnv(_, closure_local, index, val_local) = instr.kind {
                closure_set_envs.insert((closure_local, index), (val_local, bb_id, expr_idx));
            }
        }
    }

    // TODO: オーバーフローを考慮
    let mut rpo_nodes = func.bbs.keys().collect::<Vec<_>>();
    rpo_nodes.sort_by_key(|id| rpo.get(id).unwrap());

    for bb_id in &rpo_nodes {
        for expr_idx in 0..func.bbs[*bb_id].instrs.len() {
            let instr = &func.bbs[*bb_id].instrs[expr_idx];
            match instr.kind {
                InstrKind::AddInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Int(a + b);
                }
                InstrKind::SubInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Int(a - b);
                }
                InstrKind::MulInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Int(a * b);
                }
                InstrKind::DivInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2)
                        && b != 0 =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Int(a / b);
                }
                InstrKind::EqInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a == b);
                }
                InstrKind::LtInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a < b);
                }
                InstrKind::GtInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a > b);
                }
                InstrKind::LeInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a <= b);
                }
                InstrKind::GeInt(local1, local2)
                    if let Some(&InstrKind::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a >= b);
                }
                InstrKind::Not(local)
                    if let Some(&InstrKind::Bool(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(!a);
                }
                InstrKind::And(local1, local2) => {
                    let expr1 = def_use.get_def_non_move_expr(&func.bbs, local1);
                    let expr2 = def_use.get_def_non_move_expr(&func.bbs, local2);
                    match (expr1, expr2) {
                        (Some(&InstrKind::Bool(a)), Some(&InstrKind::Bool(b))) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a && b);
                        }
                        (Some(&InstrKind::Bool(false)), _) | (_, Some(&InstrKind::Bool(false))) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(false);
                        }
                        (Some(&InstrKind::Bool(true)), _) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(local2);
                        }
                        (_, Some(&InstrKind::Bool(true))) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(local1);
                        }
                        _ => {}
                    }
                }
                InstrKind::Or(local1, local2) => {
                    let expr1 = def_use.get_def_non_move_expr(&func.bbs, local1);
                    let expr2 = def_use.get_def_non_move_expr(&func.bbs, local2);
                    match (expr1, expr2) {
                        (Some(&InstrKind::Bool(a)), Some(&InstrKind::Bool(b))) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a || b);
                        }
                        (Some(&InstrKind::Bool(true)), _) | (_, Some(&InstrKind::Bool(true))) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(true);
                        }
                        (Some(&InstrKind::Bool(false)), _) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(local2);
                        }
                        (_, Some(&InstrKind::Bool(false))) => {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(local1);
                        }
                        _ => {}
                    }
                }
                InstrKind::VariadicArgsRef(local, index)
                    if let Some(InstrKind::VariadicArgs(args)) =
                        def_use.get_def_non_move_expr(&func.bbs, local)
                        && index < args.len() =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(args[index]);
                }
                InstrKind::VariadicArgsLength(local)
                    if let Some(InstrKind::VariadicArgs(args)) =
                        def_use.get_def_non_move_expr(&func.bbs, local) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Int(args.len() as i64);
                }
                InstrKind::VectorLength(local)
                    if let Some(InstrKind::Vector(elements)) =
                        def_use.get_def_non_move_expr(&func.bbs, local) =>
                {
                    // Vectorは可変だが長さは変わらないので定数畳み込みできる
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Int(elements.len() as i64);
                }
                InstrKind::ToObj(typ1, src)
                    if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                        && let InstrKind::FromObj(typ2, obj_src) = *src_expr
                        && typ1 == typ2 =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(obj_src);
                }
                InstrKind::FromObj(typ1, src)
                    if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                        && let InstrKind::ToObj(typ2, obj_src) = *src_expr
                        && typ1 == typ2 =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(obj_src);
                }
                InstrKind::Is(typ1, src)
                    if let Some(&InstrKind::ToObj(typ2, _)) =
                        def_use.get_def_non_move_expr(&func.bbs, src) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(typ1 == typ2);
                }
                InstrKind::ClosureEnv(_, closure, index)
                    if let Some(InstrKind::Closure { envs, .. }) =
                        def_use.get_def_non_move_expr(&func.bbs, closure) =>
                {
                    let env = envs[index];
                    if let Some(env) = env {
                        func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(env)
                    } else if let Some(&(env, bb_id2, expr_idx2)) =
                        closure_set_envs.get(&(closure, index))
                        && doms[bb_id].contains(&bb_id2)
                        && (bb_id2 != *bb_id || expr_idx2 < expr_idx)
                    {
                        func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(env);
                    }
                }
                InstrKind::EqObj(local1, local2)
                    if let Some(&InstrKind::ToObj(typ1, src1)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&InstrKind::ToObj(typ2, src2)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    if typ1 != typ2 {
                        func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(false);
                    } else if typ1 == ValType::Bool {
                        if let Some(&InstrKind::Bool(a)) =
                            def_use.get_def_non_move_expr(&func.bbs, src1)
                            && let Some(&InstrKind::Bool(b)) =
                                def_use.get_def_non_move_expr(&func.bbs, src2)
                        {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a == b);
                        }
                    } else if typ1 == ValType::Nil {
                        func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(true);
                    } else if typ1 == ValType::Int {
                        // Int参照の中身が同じ時のEqの結果は未規定なので畳み込んでもよい
                        if let Some(&InstrKind::Int(a)) =
                            def_use.get_def_non_move_expr(&func.bbs, src1)
                            && let Some(&InstrKind::Int(b)) =
                                def_use.get_def_non_move_expr(&func.bbs, src2)
                        {
                            func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Bool(a == b);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SsaOptimizerConfig {
    pub enable_cse: bool,
    pub enable_dce: bool,
    pub enable_inlining: bool,
}

impl Default for SsaOptimizerConfig {
    fn default() -> Self {
        SsaOptimizerConfig {
            enable_cse: true,
            enable_dce: true,
            enable_inlining: true,
        }
    }
}

pub fn ssa_optimize(func: &mut Func, config: SsaOptimizerConfig) {
    let mut def_use = DefUseChain::from_bbs(&func.bbs);
    let rpo = calculate_rpo(&func.bbs, func.bb_entry);
    let predecessors = calc_predecessors(&func.bbs);
    let doms = calc_doms(&func.bbs, &rpo, func.bb_entry, &predecessors);
    let dom_tree = build_dom_tree(&func.bbs, &rpo, func.bb_entry, &doms);

    for _ in 0..5 {
        debug_assert_ssa(func);
        copy_propagation(func, &rpo);
        eliminate_redundant_obj(func, &def_use);
        constant_folding(func, &rpo, &def_use, &doms);
        if config.enable_cse {
            common_subexpression_elimination(func, &dom_tree);
        }
    }

    if config.enable_dce {
        dead_code_elimination(func, &mut def_use);
    }
}

pub fn inlining(module: &mut Module) {
    let mut funcs = VecMap::new();
    let inliner = ModuleInliner::new(module);
    for func_id in module.funcs.keys() {
        let func = inliner.inlining(func_id);
        funcs.insert_node(func);
    }

    module.funcs = funcs;
}

#[derive(Debug, Clone)]
struct ModuleInliner<'a> {
    module: &'a Module,
    func_inliners: FxHashMap<FuncId, FuncInliner>,
}

impl<'a> ModuleInliner<'a> {
    fn new(module: &'a Module) -> Self {
        let mut func_inliners = FxHashMap::default();
        for (func_id, func) in module.funcs.iter() {
            let def_use = DefUseChain::from_bbs(&func.bbs);
            let mut call_funcs = FxHashMap::default();
            for bb_id in func.bbs.keys() {
                if let BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(
                    call_closure,
                )) = &func.bbs[bb_id].next
                    // TODO: func_idはそのモジュール内にあるとは限らない
                    && let Some(InstrKind::Closure {
                        func_id: call_func_id,
                        ..
                    }) = def_use.get_def_non_move_expr(&func.bbs, call_closure.closure)
                {
                    // TODO: ここでdesugerのようなことをするのはあまりきれいではない
                    call_funcs.insert(
                        bb_id,
                        InstrCall {
                            func_id: *call_func_id,
                            args: {
                                let mut args = Vec::new();
                                args.push(call_closure.closure);
                                args.extend(&call_closure.args);
                                args
                            },
                        },
                    );
                }
            }
            func_inliners.insert(func_id, FuncInliner { call_funcs });
        }

        ModuleInliner {
            module,
            func_inliners,
        }
    }

    fn inlining(&self, func_id: FuncId) -> Func {
        // インライン展開対象の関数が再帰的に依存している関数も含む
        // 自信を呼び出している箇所がないなら、自身を含まない
        let mut required_func_ids = {
            let mut required_func_ids = FxHashSet::default();
            let mut worklist = vec![func_id];
            while let Some(current_func_id) = worklist.pop() {
                for required_func in self.func_inliners[&current_func_id].call_funcs.values() {
                    if required_func_ids.insert(required_func.func_id) {
                        worklist.push(required_func.func_id);
                    }
                }
            }
            required_func_ids
        };

        if required_func_ids.is_empty() {
            // required_func_ids={} なら自身をそのまま返すべき
            return self.module.funcs[func_id].clone();
        }

        required_func_ids.insert(func_id);

        let mut bbs = VecMap::<BasicBlockId, BasicBlock>::new();
        let mut locals = VecMap::<LocalId, Local>::new();
        let mut merge_func_infos = FxHashMap::default();

        for &required_func_id in &required_func_ids {
            let mut local_map = FxHashMap::default();
            for (local_id, &local) in self.module.funcs[required_func_id].locals.iter() {
                let new_local_id = locals.push_with(|new_local_id| Local {
                    id: new_local_id,
                    ..local
                });
                local_map.insert(local_id, new_local_id);
            }
            let mut bb_map = FxHashMap::default();
            for bb_id in self.module.funcs[required_func_id].bbs.keys() {
                let new_bb_id = bbs.allocate_key();
                bb_map.insert(bb_id, new_bb_id);
            }
            let merge_func_info = MergeFuncInfo {
                entry_bb: bbs.allocate_key(),
                args_phi_incomings: self.module.funcs[required_func_id]
                    .args
                    .iter()
                    .map(|_| vec![])
                    .collect(),
                local_map,
                bb_map,
            };
            merge_func_infos.insert(required_func_id, merge_func_info);
        }

        for &required_func_id in &required_func_ids {
            let func_inliner = &self.func_inliners[&required_func_id];
            for (bb_id, bb) in self.module.funcs[required_func_id].bbs.iter() {
                let merge_func_info = &merge_func_infos[&required_func_id];

                let mut new_bb = BasicBlock {
                    id: merge_func_info.bb_map[&bb_id],
                    ..bb.clone()
                };
                for instr in &mut new_bb.instrs {
                    if let InstrKind::Phi(incomings) = &mut instr.kind {
                        for incoming in incomings {
                            incoming.bb = merge_func_info.bb_map[&incoming.bb];
                        }
                    }
                }
                for bb_id in new_bb.next.bb_ids_mut() {
                    *bb_id = merge_func_info.bb_map[bb_id];
                }
                for (local_id, _) in new_bb.local_usages_mut() {
                    *local_id = merge_func_info.local_map[local_id];
                }

                if let Some(call) = func_inliner.call_funcs.get(&bb_id) {
                    let args = call
                        .args
                        .iter()
                        .map(|arg| merge_func_info.local_map[arg])
                        .collect::<Vec<_>>();

                    let call_func_entry_bb_id = merge_func_infos[&call.func_id].entry_bb;
                    let args_phi_incomings = &mut merge_func_infos
                        .get_mut(&call.func_id)
                        .unwrap()
                        .args_phi_incomings;
                    for (i, &arg) in args.iter().enumerate() {
                        args_phi_incomings[i].push(PhiIncomingValue {
                            local: arg,
                            bb: new_bb.id,
                        });
                    }

                    new_bb.next = BasicBlockNext::Jump(call_func_entry_bb_id);
                }

                bbs.insert_node(new_bb);
            }
        }

        // 関数全体の引数を用意
        // 関数の引数の仮想的な生成場所であるBBを追加し、そのbb_idから引数を受け取るようなphiノードを追加
        let entry_bb_id = bbs.push_with(|entry_bb_id| BasicBlock {
            id: entry_bb_id,
            instrs: vec![],
            next: BasicBlockNext::Jump(merge_func_infos[&func_id].entry_bb),
        });
        let args = {
            let mut args = Vec::new();
            for arg_local in &self.module.funcs[func_id].args {
                let new_local_id = locals.push_with(|new_local_id| Local {
                    id: new_local_id,
                    ..self.module.funcs[func_id].locals[*arg_local]
                });
                args.push(new_local_id);
            }

            let args_phi_incomings = &mut merge_func_infos
                .get_mut(&func_id)
                .unwrap()
                .args_phi_incomings;
            for (i, &arg) in args.iter().enumerate() {
                args_phi_incomings[i].push(PhiIncomingValue {
                    local: arg,
                    bb: entry_bb_id,
                });
            }

            args
        };

        for (merge_func_id, merge_func_info) in &merge_func_infos {
            bbs.insert_node(BasicBlock {
                id: merge_func_info.entry_bb,
                instrs: merge_func_info
                    .args_phi_incomings
                    .iter()
                    .enumerate()
                    .map(|(i, incomings)| Instr {
                        local: Some(
                            merge_func_info.local_map[&self.module.funcs[*merge_func_id].args[i]],
                        ),
                        kind: InstrKind::Phi(incomings.clone()),
                    })
                    .collect(),
                next: BasicBlockNext::Jump(
                    merge_func_info.bb_map[&self.module.funcs[*merge_func_id].bb_entry],
                ),
            });
        }

        Func {
            id: func_id,
            bb_entry: entry_bb_id,
            locals,
            ret_type: self.module.funcs[func_id].ret_type,
            args,
            bbs,
        }
    }
}

#[derive(Debug, Clone)]
struct FuncInliner {
    // あるBBの末尾がCallClosureかつ、FuncIdを静的に特定できる場合のId
    call_funcs: FxHashMap<BasicBlockId, InstrCall>,
}

#[derive(Debug, Clone)]
struct MergeFuncInfo {
    // argsのphiノード用
    entry_bb: BasicBlockId,
    args_phi_incomings: Vec<Vec<PhiIncomingValue>>,
    local_map: FxHashMap<LocalId, LocalId>,
    bb_map: FxHashMap<BasicBlockId, BasicBlockId>,
}
