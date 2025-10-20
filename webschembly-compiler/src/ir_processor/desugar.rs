use std::mem;
use vec_map::VecMap;
use webschembly_compiler_ir::*;

pub fn desugar(func: &mut Func) {
    for bb in func.bbs.values_mut() {
        desugar_bb(bb, &mut func.locals);
    }
}

fn desugar_bb(bb: &mut BasicBlock, locals: &mut VecMap<LocalId, Local>) {
    let mut new_instrs = Vec::new();
    for instr in mem::take(&mut bb.instrs) {
        match instr {
            Instr {
                local,
                kind: InstrKind::Nop,
            } => {
                debug_assert!(local.is_none());
            }
            Instr {
                local,
                kind: InstrKind::CallClosure(call_closure),
            } => {
                let call_ref = desugar_call_closure(call_closure, locals, &mut new_instrs);
                new_instrs.push(Instr {
                    local,
                    kind: InstrKind::CallRef(call_ref),
                });
            }
            instr => {
                new_instrs.push(instr);
            }
        }
    }

    let dummy_next = BasicBlockNext::Jump(BasicBlockId::from(0));
    bb.next = match mem::replace(&mut bb.next, dummy_next) {
        BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(call_closure)) => {
            let call_ref = desugar_call_closure(call_closure, locals, &mut new_instrs);
            BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(call_ref))
        }
        next => next,
    };

    bb.instrs = new_instrs;
}

fn desugar_call_closure(
    call_closure: InstrCallClosure,
    locals: &mut VecMap<LocalId, Local>,
    new_instrs: &mut Vec<Instr>,
) -> InstrCallRef {
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
        typ: LocalType::FuncRef,
    });
    new_instrs.push(Instr {
        local: Some(entrypoint_table_local),
        kind: InstrKind::ClosureEntrypointTable(call_closure.closure),
    });
    new_instrs.push(Instr {
        local: Some(mut_func_ref_local),
        kind: InstrKind::EntrypointTableRef(call_closure.func_index, entrypoint_table_local),
    });
    new_instrs.push(Instr {
        local: Some(func_ref_local),
        kind: InstrKind::DerefMutFuncRef(mut_func_ref_local),
    });
    InstrCallRef {
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
