use rustc_hash::FxHashMap;
use vec_map::VecMap;
use webschembly_compiler_ir::*;

use crate::ir_processor::optimizer::remove_unreachable_bb;

const LIMIT_BB_COUNT: usize = 100;

pub fn inline_module(module: &mut Module) {
    let mut global_map = FxHashMap::default();
    let entry_func = &module.funcs[module.entry];
    // Scan entry function for GlobalSet with ConstantClosure
    for bb in entry_func.bbs.values() {
        for instr in &bb.instrs {
            if let InstrKind::GlobalSet(global_id, val_local) = instr.kind
                && let LocalType::Type(Type::Val(ValType::Closure(Some(constant)))) =
                    entry_func.locals[val_local].typ
            {
                global_map.insert(global_id, constant);
            }
        }
    }

    let mut new_funcs = VecMap::new();

    for (func_id_usize, func) in module.funcs.iter() {
        let _func_id = func_id_usize;
        let mut new_func = func.clone();
        run_inlining(&mut new_func, module, &global_map);
        new_funcs.insert(func_id_usize, new_func);
    }

    module.funcs = new_funcs;
}

struct InlineContext<'a> {
    module: &'a Module,
    tail_instances: FxHashMap<ConstantClosure, TailCallInfo>,
}

#[derive(Debug, Clone)]
struct TailCallInfo {
    entry_bb: BasicBlockId,
    arg_phis: Vec<LocalId>,
}

fn run_inlining(
    func: &mut Func,
    module: &Module,
    global_map: &FxHashMap<GlobalId, ConstantClosure>,
) {
    let mut ctx = InlineContext {
        module,
        tail_instances: FxHashMap::default(),
    };

    let mut worklist: Vec<BasicBlockId> = func.bbs.keys().collect();

    while let Some(bb_id) = worklist.pop() {
        if func.bbs.iter().count() > LIMIT_BB_COUNT {
            log::debug!("BB limit reached for func {:?}", func.id);
            break;
        }
        if !func.bbs.contains_key(bb_id) {
            continue;
        }

        // 1. Check Non-Tail Calls (Instrs)
        let mut call_found = None;
        {
            let bb = &func.bbs[bb_id];
            for (idx, instr) in bb.instrs.iter().enumerate() {
                if let InstrKind::CallClosure(call) = &instr.kind {
                    let closure_local = call.closure;
                    let mut constant_opt = None;

                    if let LocalType::Type(Type::Val(ValType::Closure(Some(constant)))) =
                        func.locals[closure_local].typ
                    {
                        constant_opt = Some(constant);
                    } else if let Some(def_instr) = find_local_def(func, closure_local)
                        && let InstrKind::GlobalGet(global_id) = def_instr.kind
                        && let Some(&constant) = global_map.get(&global_id)
                    {
                        constant_opt = Some(constant);
                    }

                    if let Some(constant) = constant_opt {
                        call_found = Some((idx, constant, call.clone()));
                        break;
                    }
                }
            }
        }

        if let Some((idx, constant, call)) = call_found {
            log::debug!(
                "Non-tail call candidate found in BB {:?} to func {:?}",
                bb_id,
                constant.func_id
            );
            let result_local = func.bbs[bb_id].instrs[idx].local;
            let continuation_bb_id = inline_non_tail(
                func,
                &mut ctx,
                &mut worklist,
                bb_id,
                idx,
                constant,
                &call,
                result_local,
            );
            worklist.push(continuation_bb_id);
            continue;
        }

        // 2. Check Tail Call (Terminator)
        let mut tail_call_found = None;
        {
            let bb = &func.bbs[bb_id];
            if let TerminatorInstr::Exit(ExitInstr::TailCallClosure(call)) = bb.terminator() {
                let closure_local = call.closure;
                let mut constant_opt = None;

                if let LocalType::Type(Type::Val(ValType::Closure(Some(constant)))) =
                    func.locals[closure_local].typ
                {
                    constant_opt = Some(constant);
                } else if let Some(def_instr) = find_local_def(func, closure_local)
                    && let InstrKind::GlobalGet(global_id) = def_instr.kind
                    && let Some(&constant) = global_map.get(&global_id)
                {
                    constant_opt = Some(constant);
                }

                if let Some(constant) = constant_opt {
                    tail_call_found = Some((constant, call.clone()));
                }
            }
        }

        if let Some((constant, call)) = tail_call_found {
            log::debug!(
                "Tail call candidate found in BB {:?} to func {:?}",
                bb_id,
                constant.func_id
            );
            inline_tail(func, &mut ctx, &mut worklist, bb_id, constant, &call);
            continue;
        }
    }

    log::debug!(
        "Inlining finished for func {:?}. Final BB count: {}",
        func.id,
        func.bbs.iter().count()
    );
    remove_unreachable_bb(func);

    // Verify Phi position
    for (id, bb) in func.bbs.iter() {
        let mut phi_mode = true;
        for (idx, instr) in bb.instrs.iter().enumerate() {
            if let InstrKind::Phi { .. } = instr.kind {
                if !phi_mode {
                    panic!(
                        "INLINE_MODULE_END: Phi at non-start! BB: {:?}, Index: {}, Instrs: {:#?}",
                        id, idx, bb.instrs
                    );
                }
            } else if !matches!(instr.kind, InstrKind::Nop) {
                phi_mode = false;
            }
        }
    }
}

fn find_local_def(func: &Func, local: LocalId) -> Option<&Instr> {
    // Simple scan for single definition (SSA-like), but inefficient.
    // However, for GlobalGet, it's usually near the top.
    // Optimally, use DefUseChain or similar, but we don't have it here easily.
    // We scan all blocks? No, that's too slow.
    // But typically instructions are defined before use in the same block or dominator.
    // For now, scan all instructions in all blocks (very slow!).
    // BETTER: Build a def map at start of inlining?
    // OR: Just scan the current block backwards? GlobalGet is usually in the same block for simple code.
    // Let's scan ALL blocks for now as a quick fix, optimizing later if needed.
    // Actually, `webschembly_compiler_ir` might have a helper?
    // I'll implement a simple full scan.
    for bb in func.bbs.values() {
        for instr in &bb.instrs {
            if instr.local == Some(local) {
                return Some(instr);
            }
        }
    }
    None
}

fn inline_non_tail(
    func: &mut Func,
    ctx: &mut InlineContext,
    worklist: &mut Vec<BasicBlockId>,
    caller_bb_id: BasicBlockId,
    instr_idx: usize,
    constant: ConstantClosure,
    call: &InstrCallClosure,
    result_local: Option<LocalId>,
) -> BasicBlockId {
    let continuation_bb_id = func.bbs.allocate_key();
    let continuation_bb_id = continuation_bb_id;
    let original_terminator = func.bbs[caller_bb_id].terminator().clone();

    let instrs_after = func.bbs[caller_bb_id].instrs.split_off(instr_idx + 1);
    func.bbs[caller_bb_id].instrs.pop();

    func.bbs.insert_node(BasicBlock {
        id: continuation_bb_id,
        instrs: instrs_after,
    });
    *func.bbs[continuation_bb_id].terminator_mut() = original_terminator;

    let callee_id = FuncId::from(constant.func_id);
    let callee = &ctx.module.funcs[callee_id];

    let mut local_map = FxHashMap::default();
    let mut bb_map = FxHashMap::default();

    for old_bb_id_usize in callee.bbs.keys() {
        let old_bb_id = old_bb_id_usize;
        let new_bb_id_usize = func.bbs.allocate_key();
        let new_bb_id = new_bb_id_usize;
        bb_map.insert(old_bb_id, new_bb_id);
        worklist.push(new_bb_id);
    }

    for (i, &arg_local) in callee.args.iter().enumerate() {
        let replacement = if i == 0 {
            call.closure
        } else if i - 1 < call.args.len() {
            call.args[i - 1]
        } else {
            call.args[call.args.len() - 1]
        };
        local_map.insert(arg_local, replacement);
    }

    for (local_id_usize, local) in callee.locals.iter() {
        let local_id = local_id_usize;
        if let std::collections::hash_map::Entry::Vacant(e) = local_map.entry(local_id) {
            let new_id_usize = func.locals.push_with(|id| Local {
                id,
                typ: local.typ,
                ..*local
            });
            let new_id = new_id_usize;
            e.insert(new_id);
        }
    }

    let mut phi_incomings = Vec::new();

    for (old_bb_id_usize, old_bb) in callee.bbs.iter() {
        let old_bb_id = old_bb_id_usize;
        let new_bb_id = bb_map[&old_bb_id];
        let mut new_instrs = Vec::new();

        for instr in &old_bb.instrs {
            let mut new_instr = instr.clone();
            if let Some(local) = new_instr.local
                && let Some(&mapped) = local_map.get(&local)
            {
                new_instr.local = Some(mapped);
            }
            rewrite_usages(&mut new_instr.kind, &local_map);
            for bb_ref in new_instr.kind.bb_ids_mut() {
                if let Some(&mapped) = bb_map.get(bb_ref) {
                    *bb_ref = mapped;
                }
            }
            new_instrs.push(new_instr);
        }

        new_instrs.pop();

        let mut new_terminator = old_bb.terminator().clone();
        rewrite_terminator_usages(&mut new_terminator, &local_map);
        for bb_ref in new_terminator.bb_ids_mut() {
            if let Some(&mapped) = bb_map.get(bb_ref) {
                *bb_ref = mapped;
            }
        }

        if let TerminatorInstr::Exit(ExitInstr::Return(val)) = &new_terminator {
            // Note: ExitInstr::Return contains LocalId (not Option)
            phi_incomings.push(PhiIncomingValue {
                local: *val,
                bb: new_bb_id,
            });
            new_terminator = TerminatorInstr::Jump(continuation_bb_id);
        } else if let TerminatorInstr::Exit(ExitInstr::TailCallClosure(call)) = &new_terminator {
            if let Some(dst) = result_local {
                let dst_typ = func.locals[dst].typ;
                let temp_local_usize = func.locals.push_with(|id| Local { id, typ: dst_typ });
                let temp_local = temp_local_usize;

                new_instrs.push(Instr {
                    local: Some(temp_local),
                    kind: InstrKind::CallClosure(call.clone()),
                });
                phi_incomings.push(PhiIncomingValue {
                    local: temp_local,
                    bb: new_bb_id,
                });
            } else {
                new_instrs.push(Instr {
                    local: None,
                    kind: InstrKind::CallClosure(call.clone()),
                });
            }
            new_terminator = TerminatorInstr::Jump(continuation_bb_id);
        }

        new_instrs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(new_terminator),
        });

        func.bbs.insert_node(BasicBlock {
            id: new_bb_id,
            instrs: new_instrs,
        });
    }

    if let Some(dst) = result_local
        && !phi_incomings.is_empty()
    {
        func.bbs[continuation_bb_id].instrs.insert(
            0,
            Instr {
                local: Some(dst),
                kind: InstrKind::Phi {
                    incomings: phi_incomings,
                    non_exhaustive: false,
                },
            },
        );
    }

    let new_entry_id = bb_map[&callee.bb_entry];

    log::debug!(
        "Inlining non-tail: Caller {:?} -> Cont {:?}, Entry {:?}",
        caller_bb_id,
        continuation_bb_id,
        new_entry_id
    );

    func.bbs[caller_bb_id].instrs.push(Instr {
        local: None,
        kind: InstrKind::Terminator(TerminatorInstr::Jump(new_entry_id)),
    });

    continuation_bb_id
}

fn inline_tail(
    func: &mut Func,
    ctx: &mut InlineContext,
    worklist: &mut Vec<BasicBlockId>,
    caller_bb_id: BasicBlockId,
    constant: ConstantClosure,
    call: &InstrCallClosure,
) {
    if let Some(info) = ctx.tail_instances.get(&constant) {
        log::debug!("Reusing tail instance at entry BB {:?}", info.entry_bb);
        for (i, &phi_local) in info.arg_phis.iter().enumerate() {
            let val = if i == 0 {
                call.closure
            } else {
                call.args[i - 1]
            };
            let entry_bb = &mut func.bbs[info.entry_bb];
            for instr in &mut entry_bb.instrs {
                if let InstrKind::Phi { incomings, .. } = &mut instr.kind
                    && instr.local == Some(phi_local)
                {
                    incomings.push(PhiIncomingValue {
                        local: val,
                        bb: caller_bb_id,
                    });
                    break;
                }
            }
        }
        *func.bbs[caller_bb_id].terminator_mut() = TerminatorInstr::Jump(info.entry_bb);
        return;
    }

    let callee_id = FuncId::from(constant.func_id);
    let callee = &ctx.module.funcs[callee_id];

    let mut local_map = FxHashMap::default();
    let mut bb_map = FxHashMap::default();

    for old_bb_id_usize in callee.bbs.keys() {
        let old_bb_id = old_bb_id_usize;
        let new_bb_id_usize = func.bbs.allocate_key();
        let new_bb_id = new_bb_id_usize;
        bb_map.insert(old_bb_id, new_bb_id);
        worklist.push(new_bb_id);
    }

    let new_entry_id = bb_map[&callee.bb_entry];
    let mut arg_phis = Vec::new();
    let mut entry_phi_instrs = Vec::new();

    for (i, &arg_local) in callee.args.iter().enumerate() {
        let new_arg_local_usize = func.locals.push_with(|id| Local {
            id,
            typ: callee.locals[arg_local].typ,
            ..callee.locals[arg_local]
        });
        let new_arg_local = new_arg_local_usize;
        local_map.insert(arg_local, new_arg_local);
        arg_phis.push(new_arg_local);

        let val = if i == 0 {
            call.closure
        } else {
            call.args[i - 1]
        };
        entry_phi_instrs.push(Instr {
            local: Some(new_arg_local),
            kind: InstrKind::Phi {
                incomings: vec![PhiIncomingValue {
                    local: val,
                    bb: caller_bb_id,
                }],
                non_exhaustive: false,
            },
        });
    }

    for (local_id_usize, local) in callee.locals.iter() {
        let local_id = local_id_usize;
        if let std::collections::hash_map::Entry::Vacant(e) = local_map.entry(local_id) {
            let new_id_usize = func.locals.push_with(|id| Local {
                id,
                typ: local.typ,
                ..*local
            });
            let new_id = new_id_usize;
            e.insert(new_id);
        }
    }

    for (old_bb_id_usize, old_bb) in callee.bbs.iter() {
        let old_bb_id = old_bb_id_usize;
        let new_bb_id = bb_map[&old_bb_id];
        let mut new_instrs = Vec::new();

        if old_bb_id == callee.bb_entry {
            new_instrs.extend(entry_phi_instrs.clone());
        }

        for instr in &old_bb.instrs {
            let mut new_instr = instr.clone();
            if let Some(local) = new_instr.local
                && let Some(&mapped) = local_map.get(&local)
            {
                new_instr.local = Some(mapped);
            }
            rewrite_usages(&mut new_instr.kind, &local_map);
            for bb_ref in new_instr.kind.bb_ids_mut() {
                if let Some(&mapped) = bb_map.get(bb_ref) {
                    *bb_ref = mapped;
                }
            }
            new_instrs.push(new_instr);
        }

        let mut new_terminator = old_bb.terminator().clone();
        rewrite_terminator_usages(&mut new_terminator, &local_map);
        for bb_ref in new_terminator.bb_ids_mut() {
            if let Some(&mapped) = bb_map.get(bb_ref) {
                *bb_ref = mapped;
            }
        }

        func.bbs.insert_node(BasicBlock {
            id: new_bb_id,
            instrs: new_instrs,
        });
        *func.bbs[new_bb_id].terminator_mut() = new_terminator;
    }

    *func.bbs[caller_bb_id].terminator_mut() = TerminatorInstr::Jump(new_entry_id);

    log::debug!(
        "Created new tail instance for func {:?} at entry BB {:?}",
        constant.func_id,
        new_entry_id
    );
    ctx.tail_instances.insert(
        constant,
        TailCallInfo {
            entry_bb: new_entry_id,
            arg_phis,
        },
    );
}

fn rewrite_usages(kind: &mut InstrKind, map: &FxHashMap<LocalId, LocalId>) {
    for (local, _) in kind.local_usages_mut() {
        if let Some(&mapped) = map.get(local) {
            *local = mapped;
        }
    }
}

fn rewrite_terminator_usages(term: &mut TerminatorInstr, map: &FxHashMap<LocalId, LocalId>) {
    for local in term.local_ids_mut() {
        if let Some(&mapped) = map.get(local) {
            *local = mapped;
        }
    }
}
