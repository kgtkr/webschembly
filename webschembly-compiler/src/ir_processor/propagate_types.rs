use crate::ir_processor::cfg_analyzer::calculate_rpo;
use vec_map::VecMap;
use webschembly_compiler_ir::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LatticeValue {
    Top,
    Constant(ConstantClosure),
    Bottom,
}

impl LatticeValue {
    fn meet(self, other: LatticeValue) -> LatticeValue {
        match (self, other) {
            (LatticeValue::Top, x) | (x, LatticeValue::Top) => x,
            (LatticeValue::Constant(a), LatticeValue::Constant(b)) => {
                if a == b {
                    LatticeValue::Constant(a)
                } else {
                    LatticeValue::Bottom
                }
            }
            (LatticeValue::Bottom, _) | (_, LatticeValue::Bottom) => LatticeValue::Bottom,
        }
    }
}

pub fn propagate_types(func: &mut Func) {
    let mut lattice = VecMap::new();
    // Initialize args to Bottom (we don't know anything about them)
    // Other locals are implicitly Top (not in map or handled as default)
    for &arg in &func.args {
        lattice.insert(arg, LatticeValue::Bottom);
    }

    // Initialize all other locals to Top
    for local_id in func.locals.keys() {
        if !lattice.contains_key(local_id) {
            lattice.insert(local_id, LatticeValue::Top);
        }
    }

    let rpo = calculate_rpo(&func.bbs, func.bb_entry);

    // Convert RPO map to a list of BBs sorted by RPO index
    let mut sorted_bbs: Vec<BasicBlockId> = rpo.keys().cloned().collect();
    sorted_bbs.sort_by_key(|bb| rpo.get(bb).unwrap());

    let mut changed = true;
    while changed {
        changed = false;

        for &bb_id in &sorted_bbs {
            let bb = &func.bbs[bb_id];
            for instr in &bb.instrs {
                if let Some(dest) = instr.local {
                    let old_val = *lattice.get(dest).unwrap_or(&LatticeValue::Top);
                    let new_val = match &instr.kind {
                        /*
                        InstrKind::Closure {
                            func_id, env_index, ..
                        } => LatticeValue::Constant(ConstantClosure {
                            func_id: *func_id,
                            env_index: ClosureEnvIndex(*env_index),
                        }),
                        */
                        InstrKind::Move(src) => *lattice.get(*src).unwrap_or(&LatticeValue::Top),
                        InstrKind::Phi {
                            incomings,
                            non_exhaustive,
                        } => {
                            if *non_exhaustive {
                                LatticeValue::Bottom
                            } else {
                                let mut val = LatticeValue::Top;
                                for incoming in incomings {
                                    let incoming_val =
                                        *lattice.get(incoming.local).unwrap_or(&LatticeValue::Top);
                                    val = val.meet(incoming_val);
                                }
                                val
                            }
                        }
                        // If we have a direct reference to a known constant logic in other instructions
                        // we might want to propagate that, but for now we only care about propagating definitions.
                        // Any other instruction that defines a value produces Bottom for that value (unknown type/value)
                        _ => LatticeValue::Bottom,
                    };

                    if old_val != new_val {
                        lattice.insert(dest, new_val);
                        changed = true;
                    }
                }
            }
        }
    }

    // Update types based on lattice results
    for (local_id, val) in lattice {
        match val {
            LatticeValue::Constant(constant) => {
                let local = &mut func.locals[local_id];
                match local.typ {
                    LocalType::Type(Type::Val(ValType::Closure(_))) => {
                        local.typ = LocalType::Type(Type::Val(ValType::Closure(Some(constant))));
                    }
                    // If it was Type::Obj (upcasted), we can refine it to Closure
                    LocalType::Type(Type::Obj) => {
                        local.typ = LocalType::Type(Type::Val(ValType::Closure(Some(constant))));
                    }
                    _ => {}
                }
            }
            // If Bottom, we might need to revert to generic Closure(None) if it was previously set to something specific
            // but here we are primarily taking generic Code (Closure(None)) and refining it.
            // If the code was *already* specialized/refined, we should be careful not to overwrite it with less specific info
            // unless we are sure. But this pass is usually run to refine.
            LatticeValue::Bottom => {
                // Ensure if we failed to prove constant, it remains as generic closure if it was a closure
                /*
                Note: We don't generally downgrade types here explicitly because the lattice implies
                we only UPGRADE to Constant or stay Bottom.
                However, if this pass is run multiple times or if we have partial information,
                we might want to be safe. But currently the IR starts with Closure(None) usually.
                */
            }
            LatticeValue::Top => {
                // Dead code or uninitialized. Leave as is or set to Bottom equivalent?
                // Usually safe to ignore.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webschembly_compiler_ir::{
        BasicBlock, ExitInstr, Func, Instr, InstrKind, Local, TerminatorInstr, Type, ValType,
    };

    fn create_dummy_func() -> Func {
        Func {
            id: FuncId::from(0),
            args: vec![],
            locals: VecMap::new(),
            bbs: VecMap::new(),
            bb_entry: BasicBlockId::from(0),
            ret_type: LocalType::Type(Type::Val(ValType::Nil)),
            closure_meta: None,
        }
    }

    #[test]
    fn test_propagate_simple_move() {
        let mut func = create_dummy_func();
        let l0 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l1 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });

        let bb0 = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![
                Instr {
                    local: Some(l0),
                    kind: InstrKind::Closure {
                        envs: vec![],
                        env_types: vec![],
                        env_index: 0,
                        module_id: JitModuleId::from(0),
                        func_id: JitFuncId::from(10),  // Target ID
                        entrypoint_table: l1,          // Dummy
                        original_entrypoint_table: l1, // Dummy
                    },
                },
                Instr {
                    local: Some(l1),
                    kind: InstrKind::Move(l0),
                },
                Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::Return(l1))),
                },
            ],
        });
        func.bb_entry = bb0;

        propagate_types(&mut func);

        let t1 = func.locals[l1].typ;
        if let LocalType::Type(Type::Val(ValType::Closure(Some(c)))) = t1 {
            assert_eq!(c.func_id, JitFuncId::from(10));
            assert_eq!(c.env_index, ClosureEnvIndex(0));
        } else {
            panic!("Expected ConstantClosure, got {:?}", t1);
        }
    }

    #[test]
    fn test_propagate_phi_same() {
        let mut func = create_dummy_func();
        // l0 and l1 are same closure constant
        let l0 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l1 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l2 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        }); // Phi result

        /*
           bb0:
             l0 = closure(10, 0)
             jump bb1
           bb1:
             l1 = closure(10, 0)
             jump bb2
           bb2:
             l2 = phi(l0: bb0, l1: bb1)
        */

        let bb0 = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![], // Fill later
        });
        let bb1 = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![], // Fill later
        });
        let bb2 = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![], // Fill later
        });

        func.bbs[bb0].instrs = vec![
            Instr {
                local: Some(l0),
                kind: InstrKind::Closure {
                    envs: vec![],
                    env_types: vec![],
                    env_index: 0,
                    module_id: JitModuleId::from(0),
                    func_id: JitFuncId::from(10),
                    entrypoint_table: l0,
                    original_entrypoint_table: l0,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(bb2)),
            },
        ];

        func.bbs[bb1].instrs = vec![
            Instr {
                local: Some(l1),
                kind: InstrKind::Closure {
                    envs: vec![],
                    env_types: vec![],
                    env_index: 0,
                    module_id: JitModuleId::from(0),
                    func_id: JitFuncId::from(10),
                    entrypoint_table: l0,
                    original_entrypoint_table: l0,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(bb2)),
            },
        ];

        func.bbs[bb2].instrs = vec![
            Instr {
                local: Some(l2),
                kind: InstrKind::Phi {
                    incomings: vec![
                        PhiIncomingValue { bb: bb0, local: l0 },
                        PhiIncomingValue { bb: bb1, local: l1 },
                    ],
                    non_exhaustive: false,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::Return(l2))),
            },
        ];

        // entry -> bb0 (cond jump is hard so just make one path reachable for simple test structure or use Jump from entry)
        // Let's make entry jump to bb0 and bb1? No, just linear flow is fine?
        // Wait, for Phi to work we need valid preds.
        // Let's make entry conditional jump to bb0 and bb1.
        let l_cond = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Bool)),
        });
        func.bb_entry = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![
                Instr {
                    local: Some(l_cond),
                    kind: InstrKind::Bool(true),
                },
                Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::If(l_cond, bb0, bb1)),
                },
            ],
        });

        propagate_types(&mut func);

        let t2 = func.locals[l2].typ;
        if let LocalType::Type(Type::Val(ValType::Closure(Some(c)))) = t2 {
            assert_eq!(c.func_id, JitFuncId::from(10));
        } else {
            panic!("Expected ConstantClosure for l2, got {:?}", t2);
        }
    }

    #[test]
    fn test_propagate_phi_different() {
        let mut func = create_dummy_func();
        let l0 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l1 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l2 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });

        let bb0 = func.bbs.push_with(|id| BasicBlock { id, instrs: vec![] });
        let bb1 = func.bbs.push_with(|id| BasicBlock { id, instrs: vec![] });
        let bb2 = func.bbs.push_with(|id| BasicBlock { id, instrs: vec![] });

        func.bbs[bb0].instrs = vec![
            Instr {
                local: Some(l0),
                kind: InstrKind::Closure {
                    envs: vec![],
                    env_types: vec![],
                    env_index: 0,
                    module_id: JitModuleId::from(0),
                    func_id: JitFuncId::from(10), // ID 10
                    entrypoint_table: l0,
                    original_entrypoint_table: l0,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(bb2)),
            },
        ];

        func.bbs[bb1].instrs = vec![
            Instr {
                local: Some(l1),
                kind: InstrKind::Closure {
                    envs: vec![],
                    env_types: vec![],
                    env_index: 0,
                    module_id: JitModuleId::from(0),
                    func_id: JitFuncId::from(11), // ID 11 (Different)
                    entrypoint_table: l0,
                    original_entrypoint_table: l0,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(bb2)),
            },
        ];

        func.bbs[bb2].instrs = vec![
            Instr {
                local: Some(l2),
                kind: InstrKind::Phi {
                    incomings: vec![
                        PhiIncomingValue { bb: bb0, local: l0 },
                        PhiIncomingValue { bb: bb1, local: l1 },
                    ],
                    non_exhaustive: false,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::Return(l2))),
            },
        ];

        let l_cond = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Bool)),
        });
        func.bb_entry = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![
                Instr {
                    local: Some(l_cond),
                    kind: InstrKind::Bool(true),
                },
                Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::If(l_cond, bb0, bb1)),
                },
            ],
        });

        propagate_types(&mut func);

        let t2 = func.locals[l2].typ;
        // Should remain generic Closure(None) because input constants are different
        if let LocalType::Type(Type::Val(ValType::Closure(None))) = t2 {
            // OK
        } else {
            panic!(
                "Expected generic Closure(None) (Bottom) for l2, got {:?}",
                t2
            );
        }
    }

    #[test]
    fn test_propagate_phi_non_exhaustive() {
        let mut func = create_dummy_func();
        let l0 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l1 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });
        let l2 = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Closure(None))),
        });

        let bb0 = func.bbs.push_with(|id| BasicBlock { id, instrs: vec![] });
        let bb1 = func.bbs.push_with(|id| BasicBlock { id, instrs: vec![] });
        let bb2 = func.bbs.push_with(|id| BasicBlock { id, instrs: vec![] });

        func.bbs[bb0].instrs = vec![
            Instr {
                local: Some(l0),
                kind: InstrKind::Closure {
                    envs: vec![],
                    env_types: vec![],
                    env_index: 0,
                    module_id: JitModuleId::from(0),
                    func_id: JitFuncId::from(10), // ID 10
                    entrypoint_table: l0,
                    original_entrypoint_table: l0,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(bb2)),
            },
        ];

        func.bbs[bb1].instrs = vec![
            Instr {
                local: Some(l1),
                kind: InstrKind::Closure {
                    envs: vec![],
                    env_types: vec![],
                    env_index: 0,
                    module_id: JitModuleId::from(0),
                    func_id: JitFuncId::from(10), // ID 10 (Same as l0)
                    entrypoint_table: l0,
                    original_entrypoint_table: l0,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(bb2)),
            },
        ];

        func.bbs[bb2].instrs = vec![
            Instr {
                local: Some(l2),
                kind: InstrKind::Phi {
                    incomings: vec![
                        PhiIncomingValue { bb: bb0, local: l0 },
                        PhiIncomingValue { bb: bb1, local: l1 },
                    ],
                    // Even though l0 and l1 are same constant, non_exhaustive should make result Bottom
                    non_exhaustive: true,
                },
            },
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::Return(l2))),
            },
        ];

        let l_cond = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Val(ValType::Bool)),
        });
        func.bb_entry = func.bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![
                Instr {
                    local: Some(l_cond),
                    kind: InstrKind::Bool(true),
                },
                Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::If(l_cond, bb0, bb1)),
                },
            ],
        });

        propagate_types(&mut func);

        let t2 = func.locals[l2].typ;
        // Should remain generic Closure(None) because non_exhaustive is true
        if let LocalType::Type(Type::Val(ValType::Closure(None))) = t2 {
            // OK
        } else {
            panic!(
                "Expected generic Closure(None) (Bottom) for l2 due to non_exhaustive, got {:?}",
                t2
            );
        }
    }
}
