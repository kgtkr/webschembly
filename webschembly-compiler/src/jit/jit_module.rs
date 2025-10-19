use rustc_hash::{FxHashMap, FxHashSet};

use super::jit_ctx::JitCtx;
use crate::fxbihashmap::FxBiHashMap;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::bb_optimizer::TypedObj;
use crate::ir_processor::cfg_analyzer::calculate_rpo;
use crate::ir_processor::dataflow::{analyze_liveness, calc_def_use};
use crate::ir_processor::optimizer::remove_unreachable_bb;
use crate::ir_processor::ssa::DefUseChain;
use crate::ir_processor::ssa_optimizer::ssa_optimize;
use vec_map::{HasId, VecMap, VecMapEq};
use webschembly_compiler_ir::*;

#[derive(Debug)]
pub struct JitModule {
    module_id: ModuleId,
    module: Module,
    jit_funcs: FxHashMap<(FuncId, usize), JitFunc>,
    func_to_globals: VecMap<FuncId, GlobalId>,
    func_types: VecMap<FuncId, FuncType>,
}

impl HasId for JitModule {
    type Id = ModuleId;
    fn id(&self) -> Self::Id {
        self.module_id
    }
}

impl JitModule {
    pub fn new(global_manager: &mut GlobalManager, module_id: ModuleId, module: Module) -> Self {
        let func_to_globals = module
            .funcs
            .keys()
            .map(|id| (id, global_manager.gen_global(LocalType::FuncRef).id))
            .collect::<VecMap<FuncId, _>>();

        let func_types = module
            .funcs
            .iter()
            .map(|(id, f)| (id, f.func_type()))
            .collect::<VecMap<FuncId, _>>();

        Self {
            module_id,
            module,
            jit_funcs: FxHashMap::default(),
            func_to_globals,
            func_types,
        }
    }

    pub fn generate_stub_module(
        &self,
        global_manager: &mut GlobalManager,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        // entry関数もあるので+1してる
        let mut stub_func_ids = FxHashMap::default();
        let mut funcs = VecMap::new();

        /*
        以下のようなentryを生成
        func entry() {
            set_global f0_ref f0_stub
            set_global f1_ref f1_stub

            f0_stub()
        }
        */

        for func in self.module.funcs.values() {
            /*
            以下のようなスタブを生成
            func f0_stub(x1, x2) {
                instantiate_module(f0_module);
                f0 <- get_global f0_ref
                f0(x1, x2)
            }
            */
            let mut new_locals = func.locals.clone();
            let f0_ref_local = new_locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            let id = funcs.push_with(|id| Func {
                id,
                args: func.args.clone(),
                ret_type: func.ret_type,
                locals: new_locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: vec![
                        Instr {
                            local: None,
                            kind: InstrKind::InstantiateFunc(self.module_id, func.id, 0),
                        },
                        Instr {
                            local: Some(f0_ref_local),
                            kind: InstrKind::GlobalGet(self.func_to_globals[func.id]),
                        },
                    ],
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(
                        InstrCallRef {
                            func: f0_ref_local,
                            args: func.args.clone(),
                            func_type: func.func_type(),
                        },
                    )),
                }]
                .into_iter()
                .collect(),
            });
            stub_func_ids.insert(func.id, id);
        }

        let entry_func_id = {
            // entry
            let mut locals = VecMap::new();
            let mut exprs = Vec::new();
            for func in self.module.funcs.values() {
                let func_ref_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::FuncRef,
                });

                exprs.push(Instr {
                    local: Some(func_ref_local),
                    kind: InstrKind::FuncRef(stub_func_ids[&func.id]),
                });
                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::GlobalSet(self.func_to_globals[func.id], func_ref_local),
                });
            }

            // 最初にインスタンス化されるモジュールなら初期化処理
            if !jit_ctx.is_instantiated() {
                let mut stub_globals = FxHashMap::default();
                for func_index in 0..GLOBAL_LAYOUT_MAX_SIZE {
                    let stub_global = global_manager.gen_global(LocalType::MutFuncRef);
                    stub_globals.insert(func_index, stub_global);
                    let stub_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::MutFuncRef,
                    });
                    exprs.push(Instr {
                        local: Some(stub_local),
                        kind: InstrKind::CreateEmptyMutFuncRef,
                    });
                    exprs.push(Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(stub_global.id, stub_local),
                    });
                }

                let instantiate_func_global = global_manager.gen_global(LocalType::FuncRef);
                jit_ctx.init_instantiated(stub_globals, instantiate_func_global);
            };

            funcs.push_with(|id| Func {
                id,
                args: vec![],
                ret_type: LocalType::Type(Type::Obj),
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: exprs,
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCall(InstrCall {
                        func_id: stub_func_ids[&self.module.entry],
                        args: vec![],
                    })),
                }]
                .into_iter()
                .collect(),
            })
        };
        Module {
            globals: FxHashMap::default(),
            funcs,
            entry: entry_func_id,
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }

    pub fn instantiate_func(
        &mut self,
        global_manager: &mut GlobalManager,
        func_id: FuncId,
        func_index: usize,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        let jit_func = JitFunc::new(
            self.module_id,
            global_manager,
            &self.module.funcs[func_id],
            func_index,
            jit_ctx,
        );
        self.jit_funcs.insert((func_id, func_index), jit_func);

        self.jit_funcs
            .get_mut(&(func_id, func_index))
            .unwrap()
            .generate_func_module(
                &self.func_to_globals,
                &self.func_types,
                global_manager,
                jit_ctx,
            )
    }

    pub fn instantiate_bb(
        &mut self,
        func_id: FuncId,
        func_index: usize,
        bb_id: BasicBlockId,
        index: usize,
        global_manager: &mut GlobalManager,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        let jit_func = self.jit_funcs.get_mut(&(func_id, func_index)).unwrap();
        let (module, _) = jit_func.generate_bb_module(
            &self.func_to_globals,
            &self.func_types,
            bb_id,
            index,
            global_manager,
            jit_ctx,
        );
        module
    }
}

#[derive(Debug)]
pub struct JitFunc {
    module_id: ModuleId,
    func_index: usize,
    func: Func,
    jit_bbs: VecMap<BasicBlockId, JitBB>,
}

impl JitFunc {
    pub fn new(
        module_id: ModuleId,
        global_manager: &mut GlobalManager,
        func: &Func,
        func_index: usize,
        jit_ctx: &mut JitCtx,
    ) -> Self {
        let mut func = func.clone();
        closure_func_assign_types(&mut func, func_index, jit_ctx.closure_global_layout());
        // 共通部分式除去を行うと変数の生存期間が伸びてしまい、JITでのパフォーマンスが落ちるのでここでは行わない
        ssa_optimize(&mut func, false);
        let bb_to_globals = func
            .bbs
            .keys()
            .map(|bb_id| (bb_id, global_manager.gen_global(LocalType::FuncRef)))
            .collect::<VecMap<BasicBlockId, _>>();
        let bb_infos = calculate_bb_info(&func);

        let jit_bbs = func
            .bbs
            .values()
            .map(|bb| JitBB {
                bb_id: bb.id,
                info: bb_infos[bb.id].clone(),
                bb_index_manager: BBIndexManager::new(bb_to_globals[bb.id]),
            })
            .collect::<VecMap<BasicBlockId, _>>();

        Self {
            module_id,
            func_index,
            func,
            jit_bbs,
        }
    }

    pub fn generate_func_module(
        &mut self,
        func_to_globals: &VecMap<FuncId, GlobalId>,
        func_types: &VecMap<FuncId, FuncType>,
        global_manager: &mut GlobalManager,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        // entry_bbのモジュールをベースに拡張する
        let (mut module, bb_func_id) = self.generate_bb_module(
            func_to_globals,
            func_types,
            self.func.bb_entry,
            GLOBAL_LAYOUT_DEFAULT_INDEX,
            global_manager,
            jit_ctx,
        );

        let body_func_id = {
            /*
            func f0(...) {
                bb0_func(...)
            }
            */

            let entry_bb_info = &self.jit_bbs[self.func.bb_entry].info;

            let locals = self.func.locals.clone();

            module.funcs.push_with(|id| Func {
                id,
                args: self.func.args.clone(),
                ret_type: self.func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: vec![],
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCall(InstrCall {
                        func_id: bb_func_id,
                        args: entry_bb_info.args.to_vec(),
                    })),
                }]
                .into_iter()
                .collect(),
            })
        };

        extend_entry_func(&mut module, |entry_func, next| {
            entry_func.bbs.push_with(|id| {
                /*
                func entry() {
                    set_global f0_ref f0
                    set_global bb0_ref [bb0_stub, nil, ..., nil]
                    set_global bb1_ref [bb1_stub, nil, ..., nil]
                }
                */

                let func_ref_local = entry_func.locals.push_with(|id| Local {
                    id,
                    typ: LocalType::FuncRef,
                });

                let mut exprs = Vec::new();
                exprs.extend([
                    Instr {
                        local: Some(func_ref_local),
                        kind: InstrKind::FuncRef(body_func_id),
                    },
                    Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(
                            jit_ctx.instantiate_func_global().id,
                            func_ref_local,
                        ),
                    },
                ]);
                if self.func_index == GLOBAL_LAYOUT_DEFAULT_INDEX {
                    // func_to_globalsはindex=0のためのもの
                    exprs.push(Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(func_to_globals[self.func.id], func_ref_local),
                    });
                }

                BasicBlock {
                    id,
                    instrs: exprs,
                    next,
                }
            })
        });

        for bb_id in self.jit_bbs.keys() {
            self.add_bb_stub_func(
                self.module_id,
                bb_id,
                GLOBAL_LAYOUT_DEFAULT_INDEX,
                &mut module,
            );
        }

        module
    }

    fn add_bb_stub_func(
        &self,
        module_id: ModuleId,
        bb_id: BasicBlockId,
        index: usize,
        module: &mut Module,
    ) {
        let jit_bb = &self.jit_bbs[bb_id];
        let func_id = module.funcs.push_with(|id| {
            /*
            func bb0_stub(...) {
                instantiate_bb(..., index)
                bb0 <- get_global bb0_ref
                bb0(...)
            }
            */
            let (type_args, index_global) = jit_bb.bb_index_manager.type_args(index);
            let mut locals = self.func.locals.clone();
            for (type_param_id, type_arg) in type_args.iter() {
                let local = *jit_bb.info.type_params.get_by_left(&type_param_id).unwrap();
                locals[local].typ = LocalType::Type(Type::Val(*type_arg));
            }

            let func_ref_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            Func {
                id,
                args: jit_bb.info.args.clone(),
                ret_type: self.func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: vec![
                        Instr {
                            local: None,
                            kind: InstrKind::InstantiateBB(
                                module_id,
                                self.func.id,
                                self.func_index,
                                jit_bb.bb_id,
                                index,
                            ),
                        },
                        Instr {
                            local: Some(func_ref_local),
                            kind: InstrKind::GlobalGet(index_global.id),
                        },
                    ],
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(
                        InstrCallRef {
                            func: func_ref_local,
                            args: jit_bb.info.args.clone(),
                            func_type: FuncType {
                                args: jit_bb.info.arg_types(&self.func, type_args),
                                ret: self.func.ret_type,
                            },
                        },
                    )),
                }]
                .into_iter()
                .collect(),
            }
        });

        let (_, index_global) = jit_bb.bb_index_manager.type_args(index);

        extend_entry_func(module, |entry_func, next| {
            let func_ref_local = entry_func.locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });
            entry_func.bbs.push_with(|id| BasicBlock {
                id,
                instrs: vec![
                    Instr {
                        local: Some(func_ref_local),
                        kind: InstrKind::FuncRef(func_id),
                    },
                    Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(index_global.id, func_ref_local),
                    },
                ],
                next,
            })
        });
    }

    pub fn generate_bb_module(
        &mut self,
        func_to_globals: &VecMap<FuncId, GlobalId>,
        func_types: &VecMap<FuncId, FuncType>,
        orig_entry_bb_id: BasicBlockId,
        index: usize,
        global_manager: &mut GlobalManager,
        jit_ctx: &mut JitCtx,
    ) -> (Module, FuncId /* BBの実態を表す関数 */) {
        let mut required_closure_idx = Vec::new();

        {
            // entrypoint_table[0]のスタブはJS APIからも使われるので未初期化の場合作成しておく
            // TODO: generate_stub_moduleで行うべき
            let (closure_idx, flag) = jit_ctx
                .closure_global_layout()
                .idx(&ClosureArgs::Variadic)
                .unwrap();
            if flag == IndexFlag::NewInstance {
                required_closure_idx.push(closure_idx);
            }
        }

        let mut required_stubs = Vec::new();

        let (type_args, index_global) = self.jit_bbs[orig_entry_bb_id]
            .bb_index_manager
            .type_args(index);

        let mut funcs = VecMap::new();

        let body_func_id = funcs.push_with(|id| {
            // TODO: 型代入に関わらずJitBBで共通なのでそっちで処理する
            let mut body_func = self.func.clone();
            body_func.id = id;
            body_func.args = self.jit_bbs[orig_entry_bb_id].info.args.clone();
            body_func.bb_entry = orig_entry_bb_id;
            body_func
        });
        let body_func = &mut funcs[body_func_id];
        let assigned_local_to_obj = assign_type_args(
            body_func,
            &self.jit_bbs[orig_entry_bb_id].info.type_params,
            type_args,
        );
        remove_unreachable_bb(body_func); // これがないとSSAにならない
        if jit_ctx.config().enable_optimization {
            ssa_optimize(body_func, false);
        }
        let def_use_chain = DefUseChain::from_bbs(&body_func.bbs);

        let mut processed_bb_ids = FxHashSet::default();
        let mut todo_bb_ids = vec![orig_entry_bb_id];

        // マージはしないBBの一覧
        // BBに対応する関数を呼び出す
        let mut required_bbs = Vec::new();

        while let Some(orig_bb_id) = todo_bb_ids.pop() {
            if processed_bb_ids.contains(&orig_bb_id) {
                continue;
            }
            processed_bb_ids.insert(orig_bb_id);

            let new_next = match std::mem::replace(
                &mut body_func.bbs[orig_bb_id].next,
                BasicBlockNext::Jump(BasicBlockId::from(0)), // dummy
            ) {
                BasicBlockNext::If(cond, orig_then_bb_id, orig_else_bb_id) => {
                    let cond_expr = def_use_chain.get_def_non_move_expr(&body_func.bbs, cond);
                    let const_cond = if let Some(&InstrKind::Bool(b)) = cond_expr {
                        Some(b)
                    } else {
                        None
                    };

                    if let Some(const_cond) = const_cond {
                        let orig_next_bb_id = if const_cond {
                            orig_then_bb_id
                        } else {
                            orig_else_bb_id
                        };
                        todo_bb_ids.push(orig_next_bb_id);
                        BasicBlockNext::Jump(orig_next_bb_id)
                    } else {
                        let mut then_types = Vec::new();
                        let mut todo_cond_locals = vec![cond];
                        while let Some(cond_local) = todo_cond_locals.pop() {
                            if let Some(&InstrKind::Is(typ, obj_local)) =
                                def_use_chain.get_def_non_move_expr(&body_func.bbs, cond_local)
                            {
                                then_types.push((obj_local, typ));
                            } else if let Some(&InstrKind::And(cond_local1, cond_local2)) =
                                def_use_chain.get_def_non_move_expr(&body_func.bbs, cond_local)
                            {
                                todo_cond_locals.push(cond_local1);
                                todo_cond_locals.push(cond_local2);
                            }
                        }

                        required_bbs.push((orig_then_bb_id, then_types));
                        required_bbs.push((orig_else_bb_id, Vec::new()));

                        BasicBlockNext::If(cond, orig_then_bb_id, orig_else_bb_id)
                    }
                }
                BasicBlockNext::Jump(orig_next_bb_id) => {
                    todo_bb_ids.push(orig_next_bb_id);
                    BasicBlockNext::Jump(orig_next_bb_id)
                }
                BasicBlockNext::Terminator(BasicBlockTerminator::TailCall(InstrCall {
                    func_id,
                    args,
                })) => {
                    let func_ref_local = body_func.locals.push_with(|id| Local {
                        id,
                        typ: LocalType::FuncRef,
                    });
                    let instrs = &mut body_func.bbs[orig_bb_id].instrs;
                    instrs.push(Instr {
                        local: Some(func_ref_local),
                        kind: InstrKind::GlobalGet(func_to_globals[func_id]),
                    });
                    BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(InstrCallRef {
                        func: func_ref_local,
                        args,
                        func_type: func_types[func_id].clone(),
                    }))
                }
                BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(call_closure)) => {
                    let call_closure = specialize_call_closure(
                        &call_closure,
                        &def_use_chain,
                        &body_func.bbs,
                        jit_ctx.closure_global_layout(),
                        &mut required_closure_idx,
                    )
                    .unwrap_or_else(|| call_closure.clone());
                    BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(call_closure))
                }
                next @ BasicBlockNext::Terminator(
                    BasicBlockTerminator::TailCallRef(_)
                    | BasicBlockTerminator::Return(_)
                    | BasicBlockTerminator::Error(_),
                ) => next,
            };

            body_func.bbs[orig_bb_id].next = new_next;
        }

        for (bb_id, types) in required_bbs {
            let mut instrs = Vec::new();
            for instr in &body_func.bbs[bb_id].instrs {
                // ジャンプ先のBBのPhiはここに移動
                // TODO: 型代入を考慮しなくてよい理由を明記
                if let InstrKind::Phi(_) = instr.kind {
                    instrs.push(instr.clone());
                }
            }

            // この分岐で型が確定するobj
            let mut typed_objs = FxHashMap::default();
            for (obj_local, typ) in types {
                let val_local = body_func.locals.push_with(|id| Local {
                    id,
                    typ: typ.into(),
                });
                instrs.push(Instr {
                    local: Some(val_local),
                    kind: InstrKind::FromObj(typ, obj_local),
                });
                typed_objs.insert(
                    obj_local,
                    TypedObj {
                        typ,
                        val_type: val_local,
                    },
                );
            }

            let callee_jit_bb = &mut self.jit_bbs[bb_id];
            let (locals_to_pass, type_args, index_global) = calculate_args_to_pass(
                &callee_jit_bb.info,
                &typed_objs,
                &def_use_chain,
                &body_func.bbs,
                &assigned_local_to_obj,
                &mut callee_jit_bb.bb_index_manager,
                &mut required_stubs,
                global_manager,
            );

            let func_ref_local = body_func.locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            instrs.extend([Instr {
                local: Some(func_ref_local),
                kind: InstrKind::GlobalGet(index_global.id),
            }]);

            body_func.bbs[bb_id].instrs = instrs;
            body_func.bbs[bb_id].next =
                BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(InstrCallRef {
                    func: func_ref_local,
                    args: locals_to_pass,
                    func_type: FuncType {
                        args: self.jit_bbs[bb_id].info.arg_types(&self.func, &type_args),
                        ret: body_func.ret_type,
                    },
                }));
        }

        for &bb_id in &processed_bb_ids {
            let mut instrs = Vec::new();
            for instr in &body_func.bbs[bb_id].instrs {
                // FuncRefとCall命令はget global命令に置き換えられる
                match *instr {
                    Instr {
                        local,
                        kind: InstrKind::Phi(ref incomings),
                    } => {
                        if bb_id == orig_entry_bb_id {
                            // 削除
                        } else {
                            // TODO: 例えば if (true) { bb1 } else { bb2 } phi(local1 from bb1, local2 from bb2) のような場合、後続の処理でincomingを消す必要がある
                            instrs.push(Instr {
                                local,
                                kind: InstrKind::Phi(
                                    incomings
                                        .iter()
                                        .copied()
                                        .filter(|incoming| /* 後方ジャンプを考慮 */ processed_bb_ids.contains(&incoming.bb))
                                        .collect(),
                                ),
                            });
                        }
                    }
                    Instr {
                        local,
                        kind: InstrKind::FuncRef(id),
                    } => {
                        instrs.push(Instr {
                            local,
                            kind: InstrKind::GlobalGet(func_to_globals[id]),
                        });
                    }
                    Instr {
                        local,
                        kind: InstrKind::Call(InstrCall { func_id, ref args }),
                    } => {
                        let func_ref_local = body_func.locals.push_with(|id| Local {
                            id,
                            typ: LocalType::FuncRef,
                        });

                        instrs.push(Instr {
                            local: Some(func_ref_local),
                            kind: InstrKind::GlobalGet(func_to_globals[func_id]),
                        });
                        instrs.push(Instr {
                            local,
                            kind: InstrKind::CallRef(InstrCallRef {
                                func: func_ref_local,
                                args: args.clone(),
                                func_type: func_types[func_id].clone(),
                            }),
                        });
                    }
                    Instr {
                        local,
                        kind: InstrKind::EntrypointTable(_),
                    } => {
                        let mut locals = Vec::new();
                        for index in 0..GLOBAL_LAYOUT_MAX_SIZE {
                            let stub = body_func.locals.push_with(|id| Local {
                                id,
                                typ: LocalType::MutFuncRef,
                            });
                            instrs.push(Instr {
                                local: Some(stub),
                                kind: InstrKind::GlobalGet(jit_ctx.stub_global(index).id),
                            });
                            locals.push(stub);
                        }
                        instrs.push(Instr {
                            local,
                            kind: InstrKind::EntrypointTable(locals),
                        });
                    }
                    Instr {
                        local,
                        kind: InstrKind::CallClosure(ref call_closure),
                    } => {
                        let call_closure = specialize_call_closure(
                            call_closure,
                            &def_use_chain,
                            &body_func.bbs,
                            jit_ctx.closure_global_layout(),
                            &mut required_closure_idx,
                        )
                        .unwrap_or_else(|| call_closure.clone());

                        instrs.push(Instr {
                            local,
                            kind: InstrKind::CallClosure(call_closure),
                        });
                    }
                    ref instr => {
                        instrs.push(instr.clone());
                    }
                }
            }

            body_func.bbs[bb_id].instrs = instrs;
        }

        remove_unreachable_bb(body_func);

        let required_closure_idx = required_closure_idx
            .iter()
            .map(|&closure_idx| {
                let mut locals = VecMap::new();
                let mut args = Vec::new();
                let closure_local = locals.push_with(|id| Local {
                    id,
                    typ: ValType::Closure.into(),
                });
                let mut arg_locals = Vec::new();
                args.push(closure_local);

                match jit_ctx.closure_global_layout().arg_types(closure_idx) {
                    ClosureArgs::Specified(arg_types) => {
                        for &typ in arg_types.iter() {
                            let local = locals.push_with(|id| Local {
                                id,
                                typ: LocalType::Type(typ),
                            });
                            args.push(local);
                            arg_locals.push(local);
                        }
                    }
                    ClosureArgs::Variadic => {
                        let local = locals.push_with(|id| Local {
                            id,
                            typ: LocalType::VariadicArgs,
                        });
                        args.push(local);
                        arg_locals.push(local);
                    }
                }

                let module_id_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::Type(Type::Val(ValType::Int)),
                });
                let func_id_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::Type(Type::Val(ValType::Int)),
                });
                let func_ref_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::FuncRef,
                });
                let mut_func_ref_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::MutFuncRef,
                });
                let entrypoint_table_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::EntrypointTable,
                });

                let mut exprs = Vec::new();

                exprs.push(Instr {
                    local: Some(module_id_local),
                    kind: InstrKind::ClosureModuleId(closure_local),
                });
                exprs.push(Instr {
                    local: Some(func_id_local),
                    kind: InstrKind::ClosureFuncId(closure_local),
                });
                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::InstantiateClosureFunc(
                        module_id_local,
                        func_id_local,
                        closure_idx,
                    ),
                });
                exprs.push(Instr {
                    local: Some(func_ref_local),
                    kind: InstrKind::GlobalGet(jit_ctx.instantiate_func_global().id),
                });
                exprs.push(Instr {
                    local: Some(mut_func_ref_local),
                    kind: InstrKind::CreateMutFuncRef(func_ref_local),
                });
                exprs.push(Instr {
                    local: Some(entrypoint_table_local),
                    kind: InstrKind::ClosureEntrypointTable(closure_local),
                });
                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::SetEntrypointTable(
                        closure_idx,
                        entrypoint_table_local,
                        mut_func_ref_local,
                    ),
                });

                let arg_types = arg_locals.iter().map(|&local| locals[local].typ).collect();
                let stub_func_id = funcs.push_with(|id| Func {
                    id,
                    args,
                    ret_type: LocalType::Type(Type::Obj),
                    locals,
                    bb_entry: BasicBlockId::from(0),
                    bbs: [BasicBlock {
                        id: BasicBlockId::from(0),
                        instrs: exprs,
                        next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(
                            InstrCallClosure {
                                closure: closure_local,
                                args: arg_locals,
                                arg_types,
                                func_index: closure_idx,
                            },
                        )),
                    }]
                    .into_iter()
                    .collect(),
                });

                (closure_idx, stub_func_id)
            })
            .collect::<Vec<_>>();

        let entry_func_id = {
            let mut locals = VecMap::new();

            let mut bbs = VecMap::new();

            let func_ref_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            bbs.insert_node({
                let mut exprs = vec![
                    Instr {
                        local: Some(func_ref_local),
                        kind: InstrKind::FuncRef(body_func_id),
                    },
                    Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(index_global.id, func_ref_local),
                    },
                ];

                for &(closure_idx, stub_func_id) in required_closure_idx.iter() {
                    let stub_func_ref_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::FuncRef,
                    });
                    let stub_mut_func_ref_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::MutFuncRef,
                    });
                    exprs.push(Instr {
                        local: Some(stub_func_ref_local),
                        kind: InstrKind::FuncRef(stub_func_id),
                    });
                    exprs.push(Instr {
                        local: Some(stub_mut_func_ref_local),
                        kind: InstrKind::GlobalGet(jit_ctx.stub_global(closure_idx).id),
                    });
                    exprs.push(Instr {
                        local: None,
                        kind: InstrKind::SetMutFuncRef(
                            stub_mut_func_ref_local,
                            stub_func_ref_local,
                        ),
                    });
                }

                BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: exprs,
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(func_ref_local)),
                }
            });

            funcs.push_with(|id| Func {
                id,
                args: vec![],
                ret_type: LocalType::FuncRef, // TODO: Nilでも返したほうがよさそう
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs,
            })
        };

        let mut module = Module {
            globals: FxHashMap::default(),
            funcs,
            entry: entry_func_id,
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        };

        for (bb_id, index) in &required_stubs {
            self.add_bb_stub_func(self.module_id, *bb_id, *index, &mut module);
        }

        (module, body_func_id)
    }
}

fn specialize_call_closure(
    call_closure: &InstrCallClosure,
    def_use_chain: &DefUseChain,
    bbs: &VecMap<BasicBlockId, BasicBlock>,
    closure_global_layout: &mut ClosureGlobalLayout,
    required_closure_idx: &mut Vec<usize>,
) -> Option<InstrCallClosure> {
    if call_closure.func_index != GLOBAL_LAYOUT_DEFAULT_INDEX {
        return None;
    }

    // func_index == GLOBAL_LAYOUT_DEFAULT_INDEX なら引数は[Args]を仮定してよい
    let InstrKind::VariadicArgs(args) =
        def_use_chain.get_def_non_move_expr(bbs, call_closure.args[0])?
    else {
        unreachable!("unexpected expr other than VariadicArgs");
    };

    let mut fixed_args = Vec::new();
    let mut fixed_arg_types = Vec::new();
    for &obj_arg in args {
        if let Some(&InstrKind::ToObj(typ, val_local)) =
            def_use_chain.get_def_non_move_expr(bbs, obj_arg)
        {
            fixed_args.push(val_local);
            fixed_arg_types.push(Type::from(typ));
        } else {
            fixed_args.push(obj_arg);
            fixed_arg_types.push(Type::Obj);
        }
    }

    let arg_types = fixed_arg_types
        .iter()
        .copied()
        .map(LocalType::Type)
        .collect();
    let (closure_index, flag) = closure_global_layout
        .idx(&ClosureArgs::Specified(fixed_arg_types))
        .unwrap_or_else(|| closure_global_layout.idx(&ClosureArgs::Variadic).unwrap());

    if flag == IndexFlag::NewInstance {
        required_closure_idx.push(closure_index);
    }
    Some(if closure_index == GLOBAL_LAYOUT_DEFAULT_INDEX {
        call_closure.clone()
    } else {
        InstrCallClosure {
            closure: call_closure.closure,
            args: fixed_args,
            arg_types,
            func_index: closure_index,
        }
    })
}

#[derive(Debug)]
struct JitBB {
    bb_id: BasicBlockId,
    info: BBInfo,
    bb_index_manager: BBIndexManager,
}

impl HasId for JitBB {
    type Id = BasicBlockId;
    fn id(&self) -> Self::Id {
        self.bb_id
    }
}

#[derive(Debug, Clone)]
struct BBInfo {
    bb_id: BasicBlockId,
    args: Vec<LocalId>,
    type_params: FxBiHashMap<TypeParamId, LocalId>,
}

impl BBInfo {
    fn arg_types(&self, func: &Func, type_args: &VecMap<TypeParamId, ValType>) -> Vec<LocalType> {
        self.args
            .iter()
            .map(|&arg| {
                if let Some(&type_param_id) = self.type_params.get_by_right(&arg)
                    && let Some(typ) = type_args.get(type_param_id).copied()
                {
                    LocalType::Type(Type::Val(typ))
                } else {
                    func.locals[arg].typ
                }
            })
            .collect::<Vec<_>>()
    }
}

impl HasId for BBInfo {
    type Id = BasicBlockId;
    fn id(&self) -> Self::Id {
        self.bb_id
    }
}

fn calculate_bb_info(func: &Func) -> VecMap<BasicBlockId, BBInfo> {
    let rpo = calculate_rpo(&func.bbs, func.bb_entry);
    let def_use = calc_def_use(&func.bbs);
    let liveness = analyze_liveness(&func.bbs, &def_use, &rpo);

    let mut bb_info = VecMap::new();

    for bb_id in func.bbs.keys() {
        let mut args = liveness
            .live_in
            .get(&bb_id)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        args.sort();

        let mut type_params = VecMap::new();
        for &arg in &args {
            if let LocalType::Type(Type::Obj) = func.locals[arg].typ {
                type_params.push(arg);
            }
        }

        let info = BBInfo {
            bb_id,
            args,
            type_params: type_params.into_iter().collect::<FxBiHashMap<_, _>>(),
        };

        bb_info.insert_node(info);
    }
    bb_info
}

#[allow(clippy::too_many_arguments)]
fn calculate_args_to_pass(
    callee: &BBInfo,
    typed_objs: &FxHashMap<LocalId, TypedObj>,
    def_use_chain: &DefUseChain,
    bbs: &VecMap<BasicBlockId, BasicBlock>,
    caller_assigned_local_to_obj: &FxBiHashMap<LocalId, LocalId>,
    bb_index_manager: &mut BBIndexManager,
    required_stubs: &mut Vec<(BasicBlockId, usize)>,
    global_manager: &mut GlobalManager,
) -> (Vec<LocalId>, VecMap<TypeParamId, ValType>, Global) {
    let mut type_args = VecMap::new();
    let mut args_to_pass = Vec::new();

    for &arg in &callee.args {
        let obj_arg = caller_assigned_local_to_obj
            .get_by_left(&arg)
            .copied()
            .unwrap_or(arg);

        let caller_args = if let Some(&type_param_id) = callee.type_params.get_by_right(&arg)
            && let Some(&InstrKind::ToObj(typ, val_local)) =
                def_use_chain.get_def_non_move_expr(bbs, obj_arg)
        {
            type_args.insert(type_param_id, typ);
            val_local
        } else if let Some(&type_param_id) = callee.type_params.get_by_right(&arg)
            && let Some(typed_obj) = typed_objs.get(&obj_arg)
        {
            type_args.insert(type_param_id, typed_obj.typ);
            typed_obj.val_type
        } else {
            obj_arg
        };

        args_to_pass.push(caller_args);
    }

    if let Some((global, index, flag)) = bb_index_manager.idx(&type_args, global_manager) {
        if flag == IndexFlag::NewInstance {
            required_stubs.push((callee.bb_id, index));
        }
        (args_to_pass, type_args, global)
    } else {
        calculate_args_to_pass(
            callee,
            // global layoutが満杯なら型パラメータなしで再計算
            // 型パラメータなしで呼び出すとto_idxの結果は必ずSomeになるので無限ループすることはない
            &FxHashMap::default(),
            &DefUseChain::new(),
            bbs,
            caller_assigned_local_to_obj,
            bb_index_manager,
            required_stubs,
            global_manager,
        )
    }
}

pub const GLOBAL_LAYOUT_MAX_SIZE: usize = 32;
pub const GLOBAL_LAYOUT_DEFAULT_INDEX: usize = 0;

#[derive(Debug)]
pub struct BBIndexManager {
    type_params_to_index: FxBiHashMap<VecMapEq<TypeParamId, ValType>, usize>,
    index_to_global: FxHashMap<usize, Global>,
}

impl BBIndexManager {
    pub fn new(global: Global) -> Self {
        let mut type_params_to_index = FxBiHashMap::default();
        let mut index_to_global = FxHashMap::default();
        type_params_to_index.insert(
            VecMapEq::from(VecMap::default()),
            GLOBAL_LAYOUT_DEFAULT_INDEX,
        );
        index_to_global.insert(GLOBAL_LAYOUT_DEFAULT_INDEX, global);
        Self {
            type_params_to_index,
            index_to_global,
        }
    }

    pub fn idx(
        &mut self,
        type_params: &VecMap<TypeParamId, ValType>,
        global_manager: &mut GlobalManager,
    ) -> Option<(Global, usize, IndexFlag)> {
        if let Some(&index) = self
            .type_params_to_index
            .get_by_left(VecMapEq::from_ref(type_params))
        {
            let global = *self.index_to_global.get(&index).unwrap();
            Some((global.to_import(), index, IndexFlag::ExistingInstance))
        } else if self.type_params_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.type_params_to_index.len();
            self.type_params_to_index
                .insert(VecMapEq::from(type_params.clone()), index);
            let global = global_manager.gen_global(LocalType::FuncRef);
            self.index_to_global.insert(index, global);
            Some((global, index, IndexFlag::NewInstance))
        } else {
            None
        }
    }

    pub fn type_args(&self, index: usize) -> (&VecMap<TypeParamId, ValType>, Global) {
        (
            self.type_params_to_index
                .get_by_right(&index)
                .unwrap()
                .as_inner(),
            self.index_to_global.get(&index).unwrap().to_import(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFlag {
    NewInstance,
    ExistingInstance,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClosureArgs {
    Specified(Vec<Type>),
    Variadic,
}

#[derive(Debug)]
pub struct ClosureGlobalLayout {
    args_to_index: FxBiHashMap<ClosureArgs, usize>,
    instantiated_idx: FxHashSet<usize>,
}

impl Default for ClosureGlobalLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl ClosureGlobalLayout {
    pub fn new() -> Self {
        let mut args_to_index = FxBiHashMap::default();
        args_to_index.insert(ClosureArgs::Variadic, GLOBAL_LAYOUT_DEFAULT_INDEX);
        Self {
            args_to_index,
            instantiated_idx: FxHashSet::default(),
        }
    }

    pub fn idx(&mut self, args: &ClosureArgs) -> Option<(usize, IndexFlag)> {
        // TODO: argsの長さに上限を設定
        let index = if let Some(&index) = self.args_to_index.get_by_left(args) {
            index
        } else if self.args_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.args_to_index.len();
            self.args_to_index.insert(args.clone(), index);
            index
        } else {
            return None;
        };
        let flag = if self.instantiated_idx.insert(index) {
            IndexFlag::NewInstance
        } else {
            IndexFlag::ExistingInstance
        };
        Some((index, flag))
    }

    pub fn arg_types(&self, index: usize) -> &ClosureArgs {
        self.args_to_index.get_by_right(&index).unwrap()
    }
}

fn closure_func_assign_types(
    func: &mut Func,
    func_index: usize,
    closure_global_layout: &ClosureGlobalLayout,
) {
    let ClosureArgs::Specified(args_type) = closure_global_layout.arg_types(func_index) else {
        return;
    };

    debug_assert_eq!(func.args.len(), 2);
    debug_assert_eq!(
        func.args
            .iter()
            .map(|&arg| func.locals[arg].typ)
            .collect::<Vec<_>>(),
        vec![
            LocalType::Type(Type::Val(ValType::Closure)),
            LocalType::VariadicArgs
        ]
    );
    debug_assert_eq!(func.ret_type, LocalType::Type(Type::Obj));

    let prev_entry = func.bb_entry;
    let variadic_args_local = func.args[1];

    let mut new_args = Vec::new();
    new_args.push(func.args[0]); // closure

    let new_bb_entry = func.bbs.push_with(|bb_id| {
        let mut exprs = Vec::new();

        let mut obj_locals = Vec::new();

        for &typ in args_type.iter() {
            let local = func.locals.push_with(|id| Local {
                id,
                typ: LocalType::Type(typ),
            });
            new_args.push(local);
            let obj_local = if let Type::Val(val_type) = typ {
                let obj_local = func.locals.push_with(|id| Local {
                    id,
                    typ: LocalType::Type(Type::Obj),
                });
                exprs.push(Instr {
                    local: Some(obj_local),
                    kind: InstrKind::ToObj(val_type, local),
                });
                obj_local
            } else {
                local
            };
            obj_locals.push(obj_local);
        }

        exprs.push(Instr {
            local: Some(variadic_args_local),
            kind: InstrKind::VariadicArgs(obj_locals),
        });

        BasicBlock {
            id: bb_id,
            instrs: exprs,
            next: BasicBlockNext::Jump(prev_entry),
        }
    });

    func.args = new_args;
    func.bb_entry = new_bb_entry;
}

// エントリー関数を拡張
// ir.rsに置くべきかも？
fn extend_entry_func(
    module: &mut Module,
    f: impl FnOnce(&mut Func, BasicBlockNext) -> BasicBlockId,
) {
    let entry_func = &mut module.funcs[module.entry];

    extend_entry_bb(entry_func, f);
}

fn extend_entry_bb(func: &mut Func, f: impl FnOnce(&mut Func, BasicBlockNext) -> BasicBlockId) {
    let prev_entry_bb_id = func.bb_entry;
    let new_entry_bb_id = f(func, BasicBlockNext::Jump(prev_entry_bb_id));
    func.bb_entry = new_entry_bb_id;
}

pub fn assign_type_args(
    func: &mut Func,
    type_params: &FxBiHashMap<TypeParamId, LocalId>,
    type_args: &VecMap<TypeParamId, ValType>,
) -> FxBiHashMap<LocalId, LocalId> {
    let mut entry_bb_instrs = Vec::new();

    // 型代入されている変数のobj版を用意(l1_objに対応)
    let mut assigned_local_to_obj = FxBiHashMap::default();

    for (type_param_id, typ) in type_args.iter() {
        let local = *type_params.get_by_left(&type_param_id).unwrap();

        // ローカル変数の型に代入
        debug_assert_eq!(func.locals[local].typ, LocalType::Type(Type::Obj));
        func.locals[local].typ = LocalType::Type(Type::Val(*typ));

        // obj版のローカル変数を用意
        let obj_local = func.locals.push_with(|id| Local {
            id,
            typ: LocalType::Type(Type::Obj),
        });
        assigned_local_to_obj.insert(local, obj_local);
        entry_bb_instrs.push(Instr {
            local: Some(obj_local),
            kind: InstrKind::ToObj(*typ, *type_params.get_by_left(&type_param_id).unwrap()),
        });
    }

    for bb in func.bbs.values_mut() {
        for (local, _) in bb.local_usages_mut() {
            if let Some(&obj_local) = assigned_local_to_obj.get_by_left(local) {
                *local = obj_local;
            }
        }
    }

    extend_entry_bb(func, |func, next| {
        func.bbs.push_with(|bb_id| BasicBlock {
            id: bb_id,
            instrs: entry_bb_instrs,
            next,
        })
    });
    assigned_local_to_obj
}
