use crate::{VecMap, ir::*, ir_processor::ssa::DefUseChain};

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

        if !expr_assign.expr.is_effectful() {
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
