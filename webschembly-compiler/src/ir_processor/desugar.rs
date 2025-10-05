use crate::{VecMap, ir::*};
use std::mem;

pub fn desugar(func: &mut Func) {
    for bb in func.bbs.values_mut() {
        desugar_bb(bb, &mut func.locals);
    }
}

fn desugar_bb(bb: &mut BasicBlock, locals: &mut VecMap<LocalId, Local>) {
    let mut new_expr_assigns = Vec::new();
    for expr_assign in mem::take(&mut bb.exprs) {
        match expr_assign {
            ExprAssign {
                local,
                expr: Expr::Nop,
            } => {
                debug_assert!(local.is_none());
            }
            ExprAssign {
                local,
                expr: Expr::CallClosure(call_closure),
            } => {
                let call_ref = desugar_call_closure(call_closure, locals, &mut new_expr_assigns);
                new_expr_assigns.push(ExprAssign {
                    local,
                    expr: Expr::CallRef(call_ref),
                });
            }
            expr_assign => {
                new_expr_assigns.push(expr_assign);
            }
        }
    }

    let dummy_next = BasicBlockNext::Jump(BasicBlockId::from(0));
    bb.next = match mem::replace(&mut bb.next, dummy_next) {
        BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(call_closure)) => {
            let call_ref = desugar_call_closure(call_closure, locals, &mut new_expr_assigns);
            BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(call_ref))
        }
        next => next,
    };

    bb.exprs = new_expr_assigns;
}

fn desugar_call_closure(
    call_closure: ExprCallClosure,
    locals: &mut VecMap<LocalId, Local>,
    new_expr_assigns: &mut Vec<ExprAssign>,
) -> ExprCallRef {
    let entrypoint_table_local = locals.push_with(|id| Local {
        id,
        typ: LocalType::EntrypointTable,
    });
    let mut_func_ref_local = locals.push_with(|id| Local {
        id,
        typ: LocalType::MutFuncRef,
    });
    let func_ref_local = locals.push_with(|id| Local {
        id,
        typ: ValType::FuncRef.into(),
    });
    new_expr_assigns.push(ExprAssign {
        local: Some(entrypoint_table_local),
        expr: Expr::ClosureEntrypointTable(call_closure.closure),
    });
    new_expr_assigns.push(ExprAssign {
        local: Some(mut_func_ref_local),
        expr: Expr::EntrypointTableRef(call_closure.func_index, entrypoint_table_local),
    });
    new_expr_assigns.push(ExprAssign {
        local: Some(func_ref_local),
        expr: Expr::DerefMutFuncRef(mut_func_ref_local),
    });
    ExprCallRef {
        func: func_ref_local,
        args: {
            let mut args = Vec::new();
            args.push(call_closure.closure);
            args.extend(call_closure.args);
            args
        },
        func_type: FuncType {
            args: {
                let mut args = Vec::new();
                args.push(ValType::Closure.into());
                args.extend(call_closure.arg_types);
                args
            },
            ret: Type::Obj.into(),
        },
    }
}
