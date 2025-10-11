use rustc_hash::FxHashMap;

use crate::{
    VecMap,
    ir::*,
    ir_processor::{
        cfg_analyzer::{DomTreeNode, build_dom_tree, calc_doms, calc_predecessors, calculate_rpo},
        ssa::{DefUseChain, debug_assert_ssa},
    },
};

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
        let expr_assign = &mut func.bbs[def.bb_id].exprs[def.expr_idx];
        expr_assign.local = None;
        def_use.remove(def.local);

        if expr_assign.expr.purelity().can_dce() {
            for (&operand, _) in expr_assign.expr.local_usages() {
                let count = &mut use_counts[operand];
                *count -= 1;
                if *count == 0
                    && let Some(def) = def_use.get_def(operand)
                {
                    worklist.push(def);
                }
            }
            expr_assign.expr = Expr::Nop;
        }
    }
}

pub fn copy_propagation(func: &mut Func, rpo: &FxHashMap<BasicBlockId, usize>) {
    let mut rpo_nodes = func.bbs.keys().collect::<Vec<_>>();
    rpo_nodes.sort_by_key(|id| rpo.get(id).unwrap());

    let mut copies = FxHashMap::default();

    for bb_id in &rpo_nodes {
        let bb = &mut func.bbs[*bb_id];

        for expr_assign in &bb.exprs {
            match *expr_assign {
                ExprAssign {
                    local: Some(dest),
                    expr: Expr::Move(src),
                } => {
                    let src = copies.get(&src).copied().unwrap_or(src);
                    copies.insert(dest, src);
                }
                ExprAssign {
                    local: Some(dest),
                    expr: Expr::Phi(ref incomings),
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
    expr_map: &mut FxHashMap<Expr, LocalId>,
) {
    let bb = &mut func.bbs[dom_tree.id];
    for expr_assign in &mut bb.exprs {
        if expr_assign.local.is_none() {
            continue;
        }
        if !expr_assign.expr.purelity().can_cse() {
            continue;
        }
        if let Some(&existing) = expr_map.get(&expr_assign.expr) {
            expr_assign.expr = Expr::Move(existing);
        } else if expr_assign.local.is_some() {
            expr_map.insert(expr_assign.expr.clone(), expr_assign.local.unwrap());
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
        match func.bbs[def.bb_id].exprs[def.expr_idx].expr {
            Expr::ToObj(typ1, src) => {
                if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                    && let Expr::FromObj(typ2, obj_src) = *src_expr
                    && typ1 == typ2
                {
                    func.bbs[def.bb_id].exprs[def.expr_idx].expr = Expr::Move(obj_src);
                }
            }
            Expr::FromObj(typ1, src) => {
                if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                    && let Expr::ToObj(typ2, obj_src) = *src_expr
                    && typ1 == typ2
                {
                    func.bbs[def.bb_id].exprs[def.expr_idx].expr = Expr::Move(obj_src);
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
        let expr_indices = (0..func.bbs[*bb_id].exprs.len()).collect::<Vec<_>>();

        for expr_idx in expr_indices {
            let expr_assign = &func.bbs[*bb_id].exprs[expr_idx];
            match expr_assign.expr {
                Expr::AddInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Int(a + b);
                }
                Expr::SubInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Int(a - b);
                }
                Expr::MulInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Int(a * b);
                }
                Expr::DivInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2)
                        && b != 0 =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Int(a / b);
                }
                Expr::EqNum(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a == b);
                }
                Expr::LtInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a < b);
                }
                Expr::GtInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a > b);
                }
                Expr::LeInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a <= b);
                }
                Expr::GeInt(local1, local2)
                    if let Some(&Expr::Int(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Int(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a >= b);
                }
                Expr::Not(local)
                    if let Some(&Expr::Bool(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(!a);
                }
                Expr::And(local1, local2) => {
                    let expr1 = def_use.get_def_non_move_expr(&func.bbs, local1);
                    let expr2 = def_use.get_def_non_move_expr(&func.bbs, local2);
                    match (expr1, expr2) {
                        (Some(&Expr::Bool(a)), Some(&Expr::Bool(b))) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a && b);
                        }
                        (Some(&Expr::Bool(false)), _) | (_, Some(&Expr::Bool(false))) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(false);
                        }
                        (Some(&Expr::Bool(true)), Some(_)) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(local2);
                        }
                        (Some(_), Some(&Expr::Bool(true))) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(local1);
                        }
                        _ => {}
                    }
                }
                Expr::Or(local1, local2) => {
                    let expr1 = def_use.get_def_non_move_expr(&func.bbs, local1);
                    let expr2 = def_use.get_def_non_move_expr(&func.bbs, local2);
                    match (expr1, expr2) {
                        (Some(&Expr::Bool(a)), Some(&Expr::Bool(b))) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a || b);
                        }
                        (Some(&Expr::Bool(true)), _) | (_, Some(&Expr::Bool(true))) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(true);
                        }
                        (Some(&Expr::Bool(false)), Some(_)) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(local2);
                        }
                        (Some(_), Some(&Expr::Bool(false))) => {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(local1);
                        }
                        _ => {}
                    }
                }
                Expr::EqNum(local1, local2)
                    if let Some(&Expr::Bool(a)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::Bool(b)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a == b);
                }
                Expr::VariadicArgsRef(local, index)
                    if let Some(Expr::VariadicArgs(args)) =
                        def_use.get_def_non_move_expr(&func.bbs, local)
                        && index < args.len() =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(args[index]);
                }
                Expr::VariadicArgsLength(local)
                    if let Some(Expr::VariadicArgs(args)) =
                        def_use.get_def_non_move_expr(&func.bbs, local) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Int(args.len() as i64);
                }
                Expr::VectorLength(local)
                    if let Some(Expr::Vector(elements)) =
                        def_use.get_def_non_move_expr(&func.bbs, local) =>
                {
                    // Vectorは可変だが長さは変わらないので定数畳み込みできる
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Int(elements.len() as i64);
                }
                Expr::ToObj(typ1, src)
                    if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                        && let Expr::FromObj(typ2, obj_src) = *src_expr
                        && typ1 == typ2 =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(obj_src);
                }
                Expr::FromObj(typ1, src)
                    if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src)
                        && let Expr::ToObj(typ2, obj_src) = *src_expr
                        && typ1 == typ2 =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(obj_src);
                }
                Expr::Is(typ1, src)
                    if let Some(&Expr::ToObj(typ2, _)) =
                        def_use.get_def_non_move_expr(&func.bbs, src) =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(typ1 == typ2);
                }
                Expr::ClosureEnv(_, closure, index)
                    if let Some(Expr::Closure { envs, .. }) =
                        def_use.get_def_non_move_expr(&func.bbs, closure)
                        && index < envs.len() =>
                {
                    func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Move(envs[index]);
                }
                Expr::EqObj(local1, local2)
                    if let Some(&Expr::ToObj(typ1, src1)) =
                        def_use.get_def_non_move_expr(&func.bbs, local1)
                        && let Some(&Expr::ToObj(typ2, src2)) =
                            def_use.get_def_non_move_expr(&func.bbs, local2) =>
                {
                    if typ1 != typ2 {
                        func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(false);
                    } else if typ1 == ValType::Bool {
                        if let Some(&Expr::Bool(a)) = def_use.get_def_non_move_expr(&func.bbs, src1)
                            && let Some(&Expr::Bool(b)) =
                                def_use.get_def_non_move_expr(&func.bbs, src2)
                        {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a == b);
                        }
                    } else if typ1 == ValType::Nil {
                        func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(true);
                    } else if typ1 == ValType::Int {
                        // Int参照の中身が同じ時のEqの結果は未規定なので畳み込んでもよい
                        if let Some(&Expr::Int(a)) = def_use.get_def_non_move_expr(&func.bbs, src1)
                            && let Some(&Expr::Int(b)) =
                                def_use.get_def_non_move_expr(&func.bbs, src2)
                        {
                            func.bbs[*bb_id].exprs[expr_idx].expr = Expr::Bool(a == b);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

// enable_cseはもう少し汎用的な方法で渡す
// for jit
pub fn ssa_optimize(func: &mut Func, enable_cse: bool) {
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
        if enable_cse {
            common_subexpression_elimination(func, &dom_tree);
        }
    }

    dead_code_elimination(func, &mut def_use);
}
