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
                if *count == 0 {
                    if let Some(def) = def_use.get_def(operand) {
                        worklist.push(def);
                    }
                }
            }
            expr_assign.expr = Expr::Nop;
        }
    }
}

pub fn copy_propagation(func: &mut Func, def_use: &DefUseChain) {
    for local in func.locals.keys() {
        let Some(def) = def_use.get_def(local) else {
            continue;
        };
        let mut current = def;
        while let ExprAssign {
            local: _,
            expr: Expr::Move(src),
        } = func.bbs[current.bb_id].exprs[current.expr_idx]
        {
            if let Some(next) = def_use.get_def(src) {
                current = next;
            } else {
                break;
            }
        }
        if current != def {
            func.bbs[def.bb_id].exprs[def.expr_idx].expr = Expr::Move(current.local);
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
        } else {
            if let Some(_) = expr_assign.local {
                expr_map.insert(expr_assign.expr.clone(), expr_assign.local.unwrap());
            }
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
        match &func.bbs[def.bb_id].exprs[def.expr_idx].expr {
            &Expr::ToObj(typ1, src) => {
                if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src) {
                    if let Expr::FromObj(typ2, obj_src) = *src_expr {
                        debug_assert_eq!(typ1, typ2);
                        func.bbs[def.bb_id].exprs[def.expr_idx].expr = Expr::Move(obj_src);
                    }
                }
            }
            &Expr::FromObj(typ1, src) => {
                if let Some(src_expr) = def_use.get_def_non_move_expr(&func.bbs, src) {
                    if let Expr::ToObj(typ2, obj_src) = *src_expr {
                        debug_assert_eq!(typ1, typ2);
                        func.bbs[def.bb_id].exprs[def.expr_idx].expr = Expr::Move(obj_src);
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn ssa_optimize(func: &mut Func) {
    let mut def_use = DefUseChain::from_bbs(&func.bbs);
    let rpo = calculate_rpo(&func.bbs, func.bb_entry);
    let predecessors = calc_predecessors(&func.bbs);
    let doms = calc_doms(&func.bbs, &rpo, func.bb_entry, &predecessors);
    let dom_tree = build_dom_tree(&func.bbs, &rpo, func.bb_entry, &doms);

    for _ in 0..5 {
        debug_assert_ssa(func);
        copy_propagation(func, &def_use);
        eliminate_redundant_obj(func, &def_use);
        common_subexpression_elimination(func, &dom_tree);
    }

    dead_code_elimination(func, &mut def_use);
}
