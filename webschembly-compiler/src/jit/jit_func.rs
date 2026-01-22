use rustc_hash::{FxHashMap, FxHashSet};

use super::bb_index_manager::{BB_LAYOUT_DEFAULT_INDEX, BBIndex, BBIndexManager};
use super::closure_global_layout::{
    CLOSURE_LAYOUT_DEFAULT_INDEX, CLOSURE_LAYOUT_MAX_SIZE, ClosureArgs, ClosureGlobalLayout,
    ClosureIndex,
};
use super::env_index_manager::{ENV_LAYOUT_DEFAULT_INDEX, EnvIndex, EnvIndexManager};
use super::index_flag::IndexFlag;
use super::jit_ctx::JitCtx;
use crate::fxbihashmap::FxBiHashMap;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::cfg_analyzer::calculate_rpo;
use crate::ir_processor::dataflow::{analyze_liveness, calc_def_use};
use crate::ir_processor::optimizer::remove_unreachable_bb;
use crate::ir_processor::ssa::{DefUseChain, build_ssa};
use crate::ir_processor::ssa_optimizer::{SsaOptimizerConfig, ssa_optimize};
use crate::jit::BlockFusionConfig;
use vec_map::{HasId, VecMap};
use webschembly_compiler_ir::*;

#[derive(Debug)]
pub struct JitFunc {
    pub jit_specialized_env_funcs: FxHashMap<EnvIndex, JitSpecializedEnvFunc>,
}

impl JitFunc {
    pub fn new(
        global_manager: &mut GlobalManager,
        module_id: JitModuleId,
        jit_ctx: &mut JitCtx,
        func: &Func,
        env_index_manager: &EnvIndexManager,
    ) -> Self {
        let mut jit_specialized_env_funcs = FxHashMap::default();
        let jit_env_func = JitSpecializedEnvFunc::new(
            module_id,
            global_manager,
            func,
            ENV_LAYOUT_DEFAULT_INDEX,
            env_index_manager,
            jit_ctx,
        );
        jit_specialized_env_funcs.insert(ENV_LAYOUT_DEFAULT_INDEX, jit_env_func);
        Self {
            jit_specialized_env_funcs,
        }
    }
}

#[derive(Debug)]
pub struct JitSpecializedEnvFunc {
    pub jit_specialized_arg_funcs: FxHashMap<ClosureIndex, JitSpecializedArgFunc>,
    pub func: Func,
}

impl JitSpecializedEnvFunc {
    pub fn new(
        module_id: JitModuleId,
        global_manager: &mut GlobalManager,
        func: &Func,
        env_index: EnvIndex,
        env_index_manager: &EnvIndexManager,
        jit_ctx: &mut JitCtx,
    ) -> Self {
        let mut func = func.clone();
        closure_func_assign_env_types(&mut func, env_index, env_index_manager);

        let mut jit_specialized_arg_funcs = FxHashMap::default();
        let jit_func = JitSpecializedArgFunc::new(
            module_id,
            global_manager,
            &func,
            env_index,
            CLOSURE_LAYOUT_DEFAULT_INDEX,
            jit_ctx,
        );
        jit_specialized_arg_funcs.insert(CLOSURE_LAYOUT_DEFAULT_INDEX, jit_func);
        Self {
            jit_specialized_arg_funcs,
            func,
        }
    }
}
#[derive(Debug)]
pub struct JitSpecializedArgFunc {
    module_id: JitModuleId,
    env_index: EnvIndex,
    func_index: ClosureIndex,
    func: Func,
    jit_bbs: VecMap<BasicBlockId, JitBB>,
}

impl JitSpecializedArgFunc {
    pub fn new(
        module_id: JitModuleId,
        global_manager: &mut GlobalManager,
        func: &Func,
        env_index: EnvIndex,
        func_index: ClosureIndex,
        jit_ctx: &mut JitCtx,
    ) -> Self {
        let mut func = func.clone();
        closure_func_assign_types(&mut func, func_index, jit_ctx.closure_global_layout());
        // 共通部分式除去を行うと変数の生存期間が伸びてしまい、JITでのパフォーマンスが落ちるのでここでは行わない
        ssa_optimize(
            &mut func,
            SsaOptimizerConfig {
                enable_cse: false,
                ..Default::default()
            },
        );
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
                branch_counter: BranchCounter::default(),
            })
            .collect::<VecMap<BasicBlockId, _>>();

        Self {
            module_id,
            env_index,
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
        env_index_managers: &mut FxHashMap<FuncId, EnvIndexManager>,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        // entry_bbのモジュールをベースに拡張する
        let mut module = self.generate_bb_module(
            func_to_globals,
            func_types,
            self.func.bb_entry,
            BB_LAYOUT_DEFAULT_INDEX,
            global_manager,
            env_index_managers,
            jit_ctx,
            false,
        );

        let body_func_id = {
            /*
            func f0(...) {
                bb0_func(...)
            }
            */

            let entry_bb_info = &self.jit_bbs[self.func.bb_entry].info;

            let mut locals = self.func.locals.clone();

            let func_ref_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            /*
            TODO: 本来であれば以下のほうが効率が良いが、BBの分岐の特殊化を行った際に関数も再生成する必要があり面倒なので、一旦TailCallではなくTailCallRefを使った実装を行う
            BasicBlock {
                id: BasicBlockId::from(0),
                instrs: vec![],
                next: TerminatorInstr::Terminator(BasicBlockTerminator::TailCall(InstrCall {
                    func_id: bb_func_id,
                    args: entry_bb_info.args.to_vec(),
                })),
            }
            */

            let (_, bb_global) = self.jit_bbs[self.func.bb_entry]
                .bb_index_manager
                .type_args(BB_LAYOUT_DEFAULT_INDEX);

            module.funcs.push_with(|id| Func {
                id,
                args: self.func.args.clone(),
                ret_type: self.func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: vec![
                        Instr {
                            local: Some(func_ref_local),
                            kind: InstrKind::GlobalGet(bb_global.id),
                        },
                        Instr {
                            local: None,
                            kind: InstrKind::Terminator(TerminatorInstr::Exit(
                                ExitInstr::TailCallRef(InstrCallRef {
                                    func: func_ref_local,
                                    args: entry_bb_info.args.to_vec(),
                                    func_type: FuncType {
                                        args: entry_bb_info
                                            .arg_types(&self.func, &VecMap::default()),
                                        ret: self.func.ret_type,
                                    },
                                }),
                            )),
                        },
                    ],
                }]
                .into_iter()
                .collect(),
                closure_meta: None,
            })
        };

        module.extend_entry_func(|entry_func, next| {
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

                if self.env_index == ENV_LAYOUT_DEFAULT_INDEX
                    && self.func_index == CLOSURE_LAYOUT_DEFAULT_INDEX
                {
                    // func_to_globalsはindex=0のためのもの
                    exprs.push(Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(func_to_globals[self.func.id], func_ref_local),
                    });
                }

                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::Terminator(next),
                });

                BasicBlock { id, instrs: exprs }
            })
        });

        for bb_id in self.jit_bbs.keys() {
            self.add_bb_stub_func(self.module_id, bb_id, BB_LAYOUT_DEFAULT_INDEX, &mut module);
        }

        module
    }

    fn add_bb_stub_func(
        &self,
        module_id: JitModuleId,
        bb_id: BasicBlockId,
        index: BBIndex,
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
                                JitFuncId::from(self.func.id),
                                self.env_index.0,
                                self.func_index.0,
                                JitBasicBlockId::from(jit_bb.bb_id),
                                index.0,
                            ),
                        },
                        Instr {
                            local: Some(func_ref_local),
                            kind: InstrKind::GlobalGet(index_global.id),
                        },
                        Instr {
                            local: None,
                            kind: InstrKind::Terminator(TerminatorInstr::Exit(
                                ExitInstr::TailCallRef(InstrCallRef {
                                    func: func_ref_local,
                                    args: jit_bb.info.args.clone(),
                                    func_type: FuncType {
                                        args: jit_bb.info.arg_types(&self.func, type_args),
                                        ret: self.func.ret_type,
                                    },
                                }),
                            )),
                        },
                    ],
                }]
                .into_iter()
                .collect(),
                closure_meta: None,
            }
        });

        let (_, index_global) = jit_bb.bb_index_manager.type_args(index);

        module.extend_entry_func(|entry_func, next| {
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
                    Instr {
                        local: None,
                        kind: InstrKind::Terminator(next),
                    },
                ],
            })
        });
    }

    pub fn generate_bb_module(
        &mut self,
        func_to_globals: &VecMap<FuncId, GlobalId>,
        func_types: &VecMap<FuncId, FuncType>,
        orig_entry_bb_id: BasicBlockId,
        index: BBIndex,
        global_manager: &mut GlobalManager,
        env_index_managers: &mut FxHashMap<FuncId, EnvIndexManager>,
        jit_ctx: &mut JitCtx,
        branch_specialization: bool,
    ) -> Module {
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
        // これがないとBBの入力に代入している命令を持つBBが残るためSSAにならない
        remove_unreachable_bb(body_func);

        let new_ids = build_ssa(body_func);

        let assigned_local_to_obj = assign_type_args(
            body_func,
            &self.jit_bbs[orig_entry_bb_id].info.type_params,
            type_args,
        );

        if jit_ctx.config().enable_optimization {
            ssa_optimize(
                body_func,
                SsaOptimizerConfig {
                    enable_cse: false, // 変数の生存期間が伸びてしまうため無効化
                    enable_dce: false, // ここでやるとmatmulが動かない
                    ..Default::default()
                },
            );
        }
        let def_use_chain = DefUseChain::from_bbs(&body_func.bbs);

        // マージ済みのBB ID、マージ予定のBB ID
        let mut processed_bb_ids = FxHashSet::default();
        let mut todo_bb_ids = vec![orig_entry_bb_id];

        // マージはしないが遅延コンパイルで呼び出すBBの一覧
        // BBに対応する関数を呼び出す
        let mut required_bbs = Vec::new();

        let mut new_entrypoint_table_globals = Vec::new();

        while let Some(orig_bb_id) = todo_bb_ids.pop() {
            if processed_bb_ids.contains(&orig_bb_id) {
                continue;
            }
            processed_bb_ids.insert(orig_bb_id);

            let new_next = match std::mem::replace(
                body_func.bbs[orig_bb_id].terminator_mut(),
                TerminatorInstr::Jump(BasicBlockId::from(0)), // dummy
            ) {
                TerminatorInstr::If(cond, orig_then_bb_id, orig_else_bb_id) => {
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
                        TerminatorInstr::Jump(orig_next_bb_id)
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

                        if branch_specialization {
                            match self.jit_bbs[orig_bb_id].branch_counter.dominant_branch() {
                                DominantBranchKind::Then => {
                                    todo_bb_ids.push(orig_then_bb_id);
                                    required_bbs.push((
                                        orig_else_bb_id,
                                        Vec::new(),
                                        BranchKind::Else,
                                    ));
                                }
                                DominantBranchKind::Else => {
                                    todo_bb_ids.push(orig_else_bb_id);
                                    required_bbs.push((
                                        orig_then_bb_id,
                                        then_types,
                                        BranchKind::Then,
                                    ));
                                }
                                DominantBranchKind::Both => match jit_ctx.config().block_fusion {
                                    BlockFusionConfig::Disabled => {
                                        unreachable!();
                                    }
                                    BlockFusionConfig::SmallFusion => {
                                        required_bbs.push((
                                            orig_then_bb_id,
                                            then_types,
                                            BranchKind::Then,
                                        ));
                                        required_bbs.push((
                                            orig_else_bb_id,
                                            Vec::new(),
                                            BranchKind::Else,
                                        ));
                                    }
                                    BlockFusionConfig::LargeFusion => {
                                        todo_bb_ids.push(orig_then_bb_id);
                                        todo_bb_ids.push(orig_else_bb_id);
                                    }
                                },
                            }
                        } else {
                            required_bbs.push((orig_then_bb_id, then_types, BranchKind::Then));
                            required_bbs.push((orig_else_bb_id, Vec::new(), BranchKind::Else));
                        }

                        TerminatorInstr::If(cond, orig_then_bb_id, orig_else_bb_id)
                    }
                }
                TerminatorInstr::Jump(orig_next_bb_id) => {
                    todo_bb_ids.push(orig_next_bb_id);
                    TerminatorInstr::Jump(orig_next_bb_id)
                }
                TerminatorInstr::Exit(ExitInstr::TailCall(InstrCall { func_id, args })) => {
                    let func_ref_local = body_func.locals.push_with(|id| Local {
                        id,
                        typ: LocalType::FuncRef,
                    });
                    let instrs = &mut body_func.bbs[orig_bb_id].instrs;
                    instrs.push(Instr {
                        local: Some(func_ref_local),
                        kind: InstrKind::GlobalGet(func_to_globals[func_id]),
                    });
                    TerminatorInstr::Exit(ExitInstr::TailCallRef(InstrCallRef {
                        func: func_ref_local,
                        args,
                        func_type: func_types[func_id].clone(),
                    }))
                }
                next @ TerminatorInstr::Exit(
                    ExitInstr::TailCallRef(_)
                    | ExitInstr::TailCallClosure(_)
                    | ExitInstr::Return(_)
                    | ExitInstr::Error(_),
                ) => next,
            };
            *body_func.bbs[orig_bb_id].terminator_mut() = new_next;
        }

        let required_bb_set = required_bbs
            .iter()
            .map(|(bb_id, _, _)| *bb_id)
            .collect::<FxHashSet<BasicBlockId>>();
        for (bb_id, types, branch_kind) in required_bbs {
            let mut instrs = Vec::new();
            for instr in &body_func.bbs[bb_id].instrs {
                // ジャンプ先のBBのPhiはここに移動
                // TODO: 型代入を考慮しなくてよい理由を明記
                if let InstrKind::Phi { .. } = instr.kind {
                    instrs.push(instr.clone());
                }
            }

            if !branch_specialization
                && jit_ctx.config().block_fusion != BlockFusionConfig::Disabled
            {
                instrs.push(Instr {
                    local: None,
                    kind: InstrKind::IncrementBranchCounter(
                        self.module_id,
                        JitFuncId::from(self.func.id),
                        self.env_index.0,
                        self.func_index.0,
                        JitBasicBlockId::from(bb_id),
                        branch_kind,
                        JitBasicBlockId::from(orig_entry_bb_id),
                        index.0,
                    ),
                });
            }

            /*
            Is命令によって分岐している場合、この分岐で型が確定する
            しかし、このbb moduleにはその後に存在するはずのfrom_obj命令が存在しないためこの情報を使った最適化が行えない
            そこで、型が確定しているならfrom_obj命令を追加して型情報を伝搬させる
            */
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
                typed_objs.insert(obj_local, TypedObj { typ, val_local });
            }

            let callee_jit_bb = &mut self.jit_bbs[bb_id];
            let (locals_to_pass, type_args, index_global) = calculate_args_to_pass(
                &callee_jit_bb.info,
                |obj_local| {
                    if let Some(&InstrKind::ToObj(typ, val_local)) =
                        def_use_chain.get_def_non_move_expr(&body_func.bbs, obj_local)
                    {
                        Some(TypedObj { typ, val_local })
                    } else {
                        typed_objs.get(&obj_local).copied()
                    }
                },
                &assigned_local_to_obj,
                &new_ids,
                &mut callee_jit_bb.bb_index_manager,
                &mut required_stubs,
                global_manager,
            );

            let func_ref_local = body_func.locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            instrs.extend([
                Instr {
                    local: Some(func_ref_local),
                    kind: InstrKind::GlobalGet(index_global.id),
                },
                Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::TailCallRef(
                        InstrCallRef {
                            func: func_ref_local,
                            args: locals_to_pass,
                            func_type: FuncType {
                                args: self.jit_bbs[bb_id].info.arg_types(&self.func, &type_args),
                                ret: body_func.ret_type,
                            },
                        },
                    ))),
                },
            ]);

            body_func.bbs[bb_id].instrs = instrs;
        }
        for &bb_id in &processed_bb_ids {
            let mut instrs = Vec::new();
            for instr in &body_func.bbs[bb_id].instrs {
                // FuncRefとCall命令はget global命令に置き換えられる
                match *instr {
                    Instr {
                        kind:
                            InstrKind::Phi {
                                ref incomings,
                                non_exhaustive,
                            },
                        ..
                    } => {
                        let new_incomings = incomings
                            .iter()
                            .filter(|incoming| !required_bb_set.contains(&incoming.bb))
                            .cloned()
                            .collect::<Vec<_>>();
                        instrs.push(Instr {
                            local: instr.local,
                            kind: InstrKind::Phi {
                                incomings: new_incomings,
                                non_exhaustive,
                            },
                        });
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
                        for index in 0..CLOSURE_LAYOUT_MAX_SIZE {
                            let stub = body_func.locals.push_with(|id| Local {
                                id,
                                typ: LocalType::MutFuncRef,
                            });
                            instrs.push(Instr {
                                local: Some(stub),
                                kind: InstrKind::GlobalGet(
                                    jit_ctx.stub_global(ClosureIndex(index)).id,
                                ),
                            });
                            locals.push(stub);
                        }
                        instrs.push(Instr {
                            local,
                            kind: InstrKind::EntrypointTable(locals),
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
        let mut all_preamble_instrs = FxHashMap::default();
        // specialize_call_closureなどの最適化はPhi命令を処理した後に行う必要があるため最後に行う
        for bb_id in body_func.bbs.keys().collect::<Vec<_>>() {
            // ClosureSetEnvを収集する
            let mut closure_set_envs = FxHashMap::default();
            for (instr_idx, instr) in body_func.bbs[bb_id].instrs.iter().enumerate() {
                if let InstrKind::ClosureSetEnv(_, closure_local, i, value_local) = instr.kind {
                    closure_set_envs.insert((closure_local, i), (value_local, instr_idx));
                }
            }

            let mut preamble_instrs = Vec::new();
            for instr_idx in 0..body_func.bbs[bb_id].instrs.len() {
                match &body_func.bbs[bb_id].instrs[instr_idx] {
                    Instr {
                        local: _,
                        kind: InstrKind::CallClosure(call_closure),
                    } if let Some(new_call_closure) = specialize_call_closure(
                        call_closure,
                        &def_use_chain,
                        &body_func.bbs,
                        jit_ctx.closure_global_layout(),
                        &mut required_closure_idx,
                    ) =>
                    {
                        body_func.bbs[bb_id].instrs[instr_idx].kind =
                            InstrKind::CallClosure(new_call_closure);
                    }
                    Instr {
                        local: _,
                        kind:
                            InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::TailCallClosure(
                                call_closure,
                            ))),
                    } if let Some(new_call_closure) = specialize_call_closure(
                        call_closure,
                        &def_use_chain,
                        &body_func.bbs,
                        jit_ctx.closure_global_layout(),
                        &mut required_closure_idx,
                    ) =>
                    {
                        body_func.bbs[bb_id].instrs[instr_idx].kind = InstrKind::Terminator(
                            TerminatorInstr::Exit(ExitInstr::TailCallClosure(new_call_closure)),
                        );
                    }
                    Instr {
                        local,
                        kind:
                            InstrKind::Closure {
                                envs,
                                env_types,
                                module_id,
                                func_id,
                                env_index: _,
                                entrypoint_table: _,
                                original_entrypoint_table,
                            },
                    } => {
                        // Noneで初期化されたenvは同じBB内のSetEnv命令のみ特殊化の対象とする
                        let mut env_types_for_manager = VecMap::new();
                        let mut new_env_types = Vec::new();
                        let mut new_envs = Vec::new();

                        // 書き換えるべきClosureSetEnv命令
                        let mut rewrite_closure_set_envs = Vec::new();
                        for (i, env_type) in env_types.iter().enumerate() {
                            if env_type == &LocalType::Type(Type::Obj)
                                && let Some(typed_obj) = envs[i]
                                    .or_else(|| {
                                        local.and_then(|local| {
                                            closure_set_envs
                                                .get(&(local, i))
                                                .map(|(val_local, _)| *val_local)
                                        })
                                    })
                                    .and_then(|value_local| {
                                        if let Some(&InstrKind::ToObj(typ, val_local)) =
                                            def_use_chain
                                                .get_def_non_move_expr(&body_func.bbs, value_local)
                                        {
                                            Some(TypedObj { typ, val_local })
                                        } else {
                                            None
                                        }
                                    })
                            {
                                env_types_for_manager.insert(i, typed_obj.typ);
                                new_env_types.push(LocalType::from(typed_obj.typ));
                                if envs[i].is_some() {
                                    new_envs.push(Some(typed_obj.val_local));
                                } else {
                                    let set_env_instr_idx =
                                        closure_set_envs.get(&(local.unwrap(), i)).unwrap().1;
                                    rewrite_closure_set_envs.push((
                                        set_env_instr_idx,
                                        local.unwrap(),
                                        i,
                                        typed_obj.val_local,
                                    ));
                                    new_envs.push(None);
                                }
                            } else {
                                new_env_types.push(*env_type);
                                new_envs.push(envs[i]);
                            }
                        }

                        let env_index_manager = env_index_managers
                            .get_mut(&FuncId::from(*func_id))
                            .expect("EnvIndexManager not found");
                        let (entrypoint_table_global, env_index, index_flag) = env_index_manager
                            .idx(&env_types_for_manager, global_manager)
                            .unwrap(); // TODO: 上限に到達したときの処理
                        if env_index != ENV_LAYOUT_DEFAULT_INDEX {
                            let entrypoint_table_global = entrypoint_table_global.unwrap();
                            if index_flag == IndexFlag::NewInstance {
                                new_entrypoint_table_globals.push(entrypoint_table_global);
                            }
                            let entrypoint_table_local = body_func.locals.push_with(|id| Local {
                                id,
                                typ: LocalType::EntrypointTable,
                            });
                            preamble_instrs.push(Instr {
                                local: Some(entrypoint_table_local),
                                kind: InstrKind::GlobalGet(entrypoint_table_global.id),
                            });
                            body_func.bbs[bb_id].instrs[instr_idx].kind = InstrKind::Closure {
                                envs: new_envs,
                                env_types: new_env_types.clone(),
                                module_id: *module_id,
                                func_id: *func_id,
                                env_index: env_index.0,
                                entrypoint_table: entrypoint_table_local,
                                original_entrypoint_table: *original_entrypoint_table,
                            };

                            for (set_env_instr_idx, closure_local, i, value_local) in
                                &rewrite_closure_set_envs
                            {
                                body_func.bbs[bb_id].instrs[*set_env_instr_idx].kind =
                                    InstrKind::ClosureSetEnv(
                                        new_env_types.clone(),
                                        *closure_local,
                                        *i,
                                        *value_local,
                                    );
                            }
                        }
                    }

                    _ => {}
                }
            }

            all_preamble_instrs.insert(bb_id, preamble_instrs);
        }

        // DefUseChainを壊さないために最後に追加する
        for (bb_id, mut preamble_instrs) in all_preamble_instrs {
            body_func.bbs[bb_id]
                .instrs
                .splice(0..0, preamble_instrs.drain(..));
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
                let env_index_local = locals.push_with(|id| Local {
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
                    local: Some(env_index_local),
                    kind: InstrKind::ClosureEnvIndex(closure_local),
                });
                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::InstantiateClosureFunc(
                        module_id_local,
                        func_id_local,
                        env_index_local,
                        closure_idx.0,
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
                        closure_idx.0,
                        entrypoint_table_local,
                        mut_func_ref_local,
                    ),
                });

                let arg_types = arg_locals.iter().map(|&local| locals[local].typ).collect();
                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::TailCallClosure(
                        InstrCallClosure {
                            closure: closure_local,
                            args: arg_locals,
                            arg_types,
                            func_index: closure_idx.0,
                        },
                    ))),
                });

                let stub_func_id = funcs.push_with(|id| Func {
                    id,
                    args,
                    ret_type: LocalType::Type(Type::Obj),
                    locals,
                    bb_entry: BasicBlockId::from(0),
                    bbs: [BasicBlock {
                        id: BasicBlockId::from(0),
                        instrs: exprs,
                    }]
                    .into_iter()
                    .collect(),
                    closure_meta: None,
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
                let mut instrs = vec![
                    Instr {
                        local: Some(func_ref_local),
                        kind: InstrKind::FuncRef(body_func_id),
                    },
                    Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(index_global.id, func_ref_local),
                    },
                ];

                for new_entrypoint_table_global in new_entrypoint_table_globals.iter() {
                    let mut entrypoint_table_locals = Vec::new();
                    for index in 0..CLOSURE_LAYOUT_MAX_SIZE {
                        let stub = locals.push_with(|id| Local {
                            id,
                            typ: LocalType::MutFuncRef,
                        });
                        instrs.push(Instr {
                            local: Some(stub),
                            kind: InstrKind::GlobalGet(jit_ctx.stub_global(ClosureIndex(index)).id),
                        });
                        entrypoint_table_locals.push(stub);
                    }
                    let entrypoint_table_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::EntrypointTable,
                    });
                    instrs.push(Instr {
                        local: Some(entrypoint_table_local),
                        kind: InstrKind::EntrypointTable(entrypoint_table_locals),
                    });
                    instrs.push(Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(
                            new_entrypoint_table_global.id,
                            entrypoint_table_local,
                        ),
                    });
                }

                for &(closure_idx, stub_func_id) in required_closure_idx.iter() {
                    let stub_func_ref_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::FuncRef,
                    });
                    let stub_mut_func_ref_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::MutFuncRef,
                    });
                    instrs.push(Instr {
                        local: Some(stub_func_ref_local),
                        kind: InstrKind::FuncRef(stub_func_id),
                    });
                    instrs.push(Instr {
                        local: Some(stub_mut_func_ref_local),
                        kind: InstrKind::GlobalGet(jit_ctx.stub_global(closure_idx).id),
                    });
                    instrs.push(Instr {
                        local: None,
                        kind: InstrKind::SetMutFuncRef(
                            stub_mut_func_ref_local,
                            stub_func_ref_local,
                        ),
                    });
                }

                instrs.push(Instr {
                    local: None,
                    kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::Return(
                        func_ref_local,
                    ))),
                });

                BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs,
                }
            });

            funcs.push_with(|id| Func {
                id,
                args: vec![],
                ret_type: LocalType::FuncRef, // TODO: Nilでも返したほうがよさそう
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs,
                closure_meta: None,
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
            self.add_bb_stub_func(self.module_id, *bb_id, BBIndex(*index), &mut module);
        }

        module
    }

    pub fn increment_branch_counter(
        &mut self,
        func_to_globals: &VecMap<FuncId, GlobalId>,
        func_types: &VecMap<FuncId, FuncType>,
        global_manager: &mut GlobalManager,
        env_index_managers: &mut FxHashMap<FuncId, EnvIndexManager>,
        jit_ctx: &mut JitCtx,
        bb_id: BasicBlockId,
        kind: BranchKind,
        source_bb_id: BasicBlockId,
        source_index: usize,
    ) -> Option<Module> {
        self.jit_bbs[bb_id].branch_counter.increment(kind);
        if self.jit_bbs[bb_id].branch_counter.should_specialize() {
            let module = self.generate_bb_module(
                func_to_globals,
                func_types,
                source_bb_id,
                BBIndex(source_index),
                global_manager,
                env_index_managers,
                jit_ctx,
                true,
            );
            Some(module)
        } else {
            None
        }
    }
}

fn specialize_call_closure(
    call_closure: &InstrCallClosure,
    def_use_chain: &DefUseChain,
    bbs: &VecMap<BasicBlockId, BasicBlock>,
    closure_global_layout: &mut ClosureGlobalLayout,
    required_closure_idx: &mut Vec<ClosureIndex>,
) -> Option<InstrCallClosure> {
    if call_closure.func_index != CLOSURE_LAYOUT_DEFAULT_INDEX.0 {
        return None;
    }

    // func_index == CLOSURE_LAYOUT_DEFAULT_INDEX なら引数は[Args]を仮定してよい
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
    Some(if closure_index == CLOSURE_LAYOUT_DEFAULT_INDEX {
        call_closure.clone()
    } else {
        InstrCallClosure {
            closure: call_closure.closure,
            args: fixed_args,
            arg_types,
            func_index: closure_index.0,
        }
    })
}

#[derive(Debug)]
struct JitBB {
    bb_id: BasicBlockId,
    info: BBInfo,
    bb_index_manager: BBIndexManager,
    // BB Indexごとにカウンターを持つと、まとめて複数の分岐をマージできないためBBごとに持つ
    branch_counter: BranchCounter,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct BranchCounter {
    pub then_count: usize,
    pub else_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DominantBranchKind {
    Then,
    Else,
    Both,
}

impl BranchCounter {
    pub fn increment(&mut self, kind: BranchKind) {
        match kind {
            BranchKind::Then => self.then_count += 1,
            BranchKind::Else => self.else_count += 1,
        }
    }

    pub fn dominant_branch(&self) -> DominantBranchKind {
        if self.then_count.checked_div(self.else_count).unwrap_or(100) >= 4 {
            DominantBranchKind::Then
        } else if self.else_count.checked_div(self.then_count).unwrap_or(100) >= 4 {
            DominantBranchKind::Else
        } else {
            DominantBranchKind::Both
        }
    }

    pub fn should_specialize(&self) -> bool {
        let total = self.then_count + self.else_count;
        total >= 20
    }
}

fn calculate_bb_info(func: &Func) -> VecMap<BasicBlockId, BBInfo> {
    let rpo = calculate_rpo(&func.bbs, func.bb_entry);
    let def_use = calc_def_use(&func.bbs);
    let liveness = analyze_liveness(&func.bbs, &def_use, &rpo);

    let mut bb_info = VecMap::new();

    for bb_id in func.bbs.keys() {
        let args = if bb_id == func.bb_entry {
            // bbを関数として使えるように
            func.args.clone()
        } else {
            let mut args = liveness
                .live_in
                .get(&bb_id)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>();
            args.sort();
            args
        };

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

// 型が確定しているobj型の情報
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedObj {
    pub val_local: LocalId,
    pub typ: ValType,
}

fn calculate_args_to_pass(
    callee: &BBInfo,
    get_typed_obj: impl Fn(LocalId) -> Option<TypedObj>,
    caller_assigned_local_to_obj: &FxHashMap<LocalId, LocalId>,
    new_ids: &FxHashMap<(BasicBlockId, LocalId), LocalId>,
    bb_index_manager: &mut BBIndexManager,
    required_stubs: &mut Vec<(BasicBlockId, usize)>,
    global_manager: &mut GlobalManager,
) -> (Vec<LocalId>, VecMap<TypeParamId, ValType>, Global) {
    let mut type_args = VecMap::new();
    let mut args_to_pass = Vec::new();
    // BBIndexManagerが満杯だったときのためのフォールバック
    let mut args_to_pass_fallback = Vec::new();

    for &arg in &callee.args {
        let arg = new_ids.get(&(callee.bb_id, arg)).copied().unwrap_or(arg);
        let obj_arg = caller_assigned_local_to_obj
            .get(&arg)
            .copied()
            .unwrap_or(arg);

        let caller_args = if let Some(&type_param_id) = callee.type_params.get_by_right(&arg)
            && let Some(typed_obj) = get_typed_obj(obj_arg)
        {
            type_args.insert(type_param_id, typed_obj.typ);
            typed_obj.val_local
        } else {
            obj_arg
        };

        args_to_pass.push(caller_args);
        args_to_pass_fallback.push(obj_arg);
    }

    let (type_args, args_to_pass, (global, index, flag)) = bb_index_manager
        .idx(&type_args, global_manager)
        .map(|x| (type_args, args_to_pass, x))
        .unwrap_or_else(|| {
            let type_args = VecMap::default();
            let x = bb_index_manager.idx(&type_args, global_manager).unwrap();
            (type_args, args_to_pass_fallback, x)
        });

    if flag == IndexFlag::NewInstance {
        required_stubs.push((callee.bb_id, index.0));
    }
    (args_to_pass, type_args, global)
}

fn closure_func_assign_env_types(
    func: &mut Func,
    env_index: EnvIndex,
    env_index_manager: &EnvIndexManager,
) {
    if env_index == ENV_LAYOUT_DEFAULT_INDEX {
        return;
    }

    /*
    // before
    func f(c: closure, ...)
        x = closure_env(c, 0)

    // after
    func f(c2: closure, ...)
        env_0 = closure_env(c2, 0)
        env_0_obj = obj<int>(env_0)
        c = closure(envs = [env_0_obj, ...], ...c2)
        x = closure_env(c, 0)
        // 最適化で良い感じになる
    */

    let closure_meta = func.closure_meta.as_ref().unwrap();
    let original_env_types = &closure_meta.env_types;
    let mut new_env_types = original_env_types.clone();
    let (assign_env_types, _) = env_index_manager.env_types(env_index);
    for (index, &val_type) in assign_env_types.iter() {
        new_env_types[index] = val_type.into();
    }
    let new_closure_arg = func.locals.push_with(|id| Local {
        id,
        typ: LocalType::Type(Type::Val(ValType::Closure)),
    });
    let prev_closure_arg = func.args[0];
    func.args[0] = new_closure_arg;

    let mut c_envs = Vec::new();
    let mut preamble_instrs = Vec::new();
    for (i, &env_type) in new_env_types.iter().enumerate() {
        let env_local = func.locals.push_with(|id| Local { id, typ: env_type });
        preamble_instrs.push(Instr {
            local: Some(env_local),
            kind: InstrKind::ClosureEnv(new_env_types.clone(), new_closure_arg, i),
        });
        if let Some(&val_type) = assign_env_types.get(i) {
            let env_obj_local = func.locals.push_with(|id| Local {
                id,
                typ: LocalType::Type(Type::Obj),
            });
            preamble_instrs.push(Instr {
                local: Some(env_obj_local),
                kind: InstrKind::ToObj(val_type, env_local),
            });
            c_envs.push(env_obj_local);
        } else {
            c_envs.push(env_local);
        }
    }

    let c_entrypoint_table_local = func.locals.push_with(|id| Local {
        id,
        typ: LocalType::EntrypointTable,
    });
    preamble_instrs.push(Instr {
        local: Some(c_entrypoint_table_local),
        kind: InstrKind::ClosureOriginalEntrypointTable(new_closure_arg),
    });

    preamble_instrs.push(Instr {
        local: Some(prev_closure_arg),
        kind: InstrKind::Closure {
            envs: c_envs.into_iter().map(Some).collect(),
            env_types: original_env_types.clone(),
            env_index: ENV_LAYOUT_DEFAULT_INDEX.0,
            module_id: closure_meta.module_id,
            func_id: closure_meta.func_id,
            entrypoint_table: c_entrypoint_table_local,
            original_entrypoint_table: c_entrypoint_table_local,
        },
    });

    func.extend_entry_bb(|func, next| {
        preamble_instrs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(next),
        });
        func.bbs.push_with(|bb_id| BasicBlock {
            id: bb_id,
            instrs: preamble_instrs,
        })
    });
}

fn closure_func_assign_types(
    func: &mut Func,
    func_index: ClosureIndex,
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

        exprs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(TerminatorInstr::Jump(prev_entry)),
        });

        BasicBlock {
            id: bb_id,
            instrs: exprs,
        }
    });

    func.args = new_args;
    func.bb_entry = new_bb_entry;
}

pub fn assign_type_args(
    func: &mut Func,
    type_params: &FxBiHashMap<TypeParamId, LocalId>,
    type_args: &VecMap<TypeParamId, ValType>,
) -> FxHashMap<LocalId, LocalId> {
    let mut entry_bb_instrs = Vec::new();

    // 型代入されている変数のobj版を用意(l1_objに対応)
    let mut assigned_local_to_obj = FxHashMap::default();

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
            if let Some(&obj_local) = assigned_local_to_obj.get(local) {
                *local = obj_local;
            }
        }
    }

    func.extend_entry_bb(|func, next| {
        entry_bb_instrs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(next),
        });
        func.bbs.push_with(|bb_id| BasicBlock {
            id: bb_id,
            instrs: entry_bb_instrs,
        })
    });
    assigned_local_to_obj
}
