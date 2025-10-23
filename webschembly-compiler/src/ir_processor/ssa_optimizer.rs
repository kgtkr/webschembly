use rustc_hash::FxHashMap;

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
) {
    // TODO: オーバーフローを考慮
    let mut rpo_nodes = func.bbs.keys().collect::<Vec<_>>();
    rpo_nodes.sort_by_key(|id| rpo.get(id).unwrap());

    for bb_id in &rpo_nodes {
        let expr_indices = (0..func.bbs[*bb_id].instrs.len()).collect::<Vec<_>>();

        for expr_idx in expr_indices {
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
                        def_use.get_def_non_move_expr(&func.bbs, closure)
                        && let Some(Some(env)) = envs.get(index) =>
                {
                    func.bbs[*bb_id].instrs[expr_idx].kind = InstrKind::Move(*env);
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
}

impl Default for SsaOptimizerConfig {
    fn default() -> Self {
        SsaOptimizerConfig {
            enable_cse: true,
            enable_dce: true,
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
        constant_folding(func, &rpo, &def_use);
        if config.enable_cse {
            common_subexpression_elimination(func, &dom_tree);
        }
    }

    if config.enable_dce {
        dead_code_elimination(func, &mut def_use);
    }
}
