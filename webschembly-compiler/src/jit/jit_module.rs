use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::{TiVec, ti_vec};

use super::bb_optimizer;
use super::bb_optimizer::TypedObj;
use super::jit_config::JitConfig;
use crate::fxbihashmap::FxBiHashMap;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::cfg_analyzer::{calc_doms, calc_predecessors, calculate_rpo};
use crate::ir_processor::dataflow::{analyze_liveness, calc_def_use};
use crate::ir_processor::ssa::{DefUseChain, collect_defs};
use crate::ir_processor::ssa_optimizer::ssa_optimize;
use crate::vec_map::VecMap;
use crate::{HasId, ir::*};

#[derive(Debug)]
pub struct JitModule {
    module_id: ModuleId,
    module: Module,
    jit_funcs: FxHashMap<(FuncId, usize), JitFunc>,
    func_to_globals: TiVec<FuncId, GlobalId>,
    func_types: VecMap<FuncId, FuncType>,
}

impl JitModule {
    pub fn new(global_manager: &mut GlobalManager, module_id: ModuleId, module: Module) -> Self {
        let func_to_globals = module
            .funcs
            .iter()
            .map(|_| global_manager.gen_global(LocalType::FuncRef))
            .collect::<TiVec<FuncId, _>>();

        let func_to_globals = func_to_globals
            .iter()
            .map(|g| g.id)
            .collect::<TiVec<FuncId, _>>();

        let func_types = module
            .funcs
            .iter()
            .map(|f| (f.id, f.func_type()))
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
        stub_globals: &mut FxHashMap<usize, Global>,
        instantiate_func_global: &mut Option<Global>,
    ) -> Module {
        // entry関数もあるので+1してる
        let stub_func_ids = self
            .module
            .funcs
            .iter()
            .map(|func| FuncId::from(usize::from(func.id) + 1))
            .collect::<TiVec<FuncId, _>>();
        let mut funcs = TiVec::<FuncId, _>::new();

        /*
        以下のようなentryを生成
        func entry() {
            set_global f0_ref f0_stub
            set_global f1_ref f1_stub

            f0_stub()
        }
        */

        {
            // entry
            let mut locals = VecMap::new();
            let mut exprs = Vec::new();
            for func in self.module.funcs.iter() {
                let func_ref_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::FuncRef,
                });

                exprs.push(ExprAssign {
                    local: Some(func_ref_local),
                    expr: Expr::FuncRef(stub_func_ids[func.id]),
                });
                exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::GlobalSet(self.func_to_globals[func.id], func_ref_local),
                });
            }

            // stub_globalsがempty(=最初にJITされるモジュール)なら、初期化を行う
            if stub_globals.is_empty() {
                for func_index in 0..GLOBAL_LAYOUT_MAX_SIZE {
                    let stub_global = global_manager.gen_global(LocalType::MutFuncRef);
                    stub_globals.insert(func_index, stub_global);
                    let stub_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::MutFuncRef,
                    });
                    exprs.push(ExprAssign {
                        local: Some(stub_local),
                        expr: Expr::CreateEmptyMutFuncRef,
                    });
                    exprs.push(ExprAssign {
                        local: None,
                        expr: Expr::GlobalSet(stub_global.id, stub_local),
                    });
                }
            };
            // instantiate_func_globalがNoneの場合も同様に初期化を行う
            if instantiate_func_global.is_none() {
                let g = global_manager.gen_global(LocalType::FuncRef);
                *instantiate_func_global = Some(g);
            }

            let func = Func {
                id: funcs.next_key(),
                args: vec![],
                ret_type: LocalType::Type(Type::Obj),
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs,
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCall(ExprCall {
                        func_id: stub_func_ids[self.module.entry],
                        args: vec![],
                    })),
                }]
                .into_iter()
                .collect(),
            };
            funcs.push(func);
        }
        for func in self.module.funcs.iter() {
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

            let func = Func {
                id: funcs.next_key(),
                args: func.args.clone(),
                ret_type: func.ret_type,
                locals: new_locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs: vec![
                        ExprAssign {
                            local: None,
                            expr: Expr::InstantiateFunc(self.module_id, func.id, 0),
                        },
                        ExprAssign {
                            local: Some(f0_ref_local),
                            expr: Expr::GlobalGet(self.func_to_globals[func.id]),
                        },
                    ],
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(
                        ExprCallRef {
                            func: f0_ref_local,
                            args: func.args.clone(),
                            func_type: func.func_type(),
                        },
                    )),
                }]
                .into_iter()
                .collect(),
            };
            funcs.push(func);
        }

        Module {
            globals: FxHashMap::default(),
            funcs,
            entry: FuncId::from(0),
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
        instantiate_func_global: Global,
        closure_global_layout: &mut ClosureGlobalLayout,
    ) -> Module {
        let jit_func = JitFunc::new(
            self.module_id,
            global_manager,
            &self.module.funcs[func_id],
            func_index,
            closure_global_layout,
        );
        self.jit_funcs.insert((func_id, func_index), jit_func);

        self.jit_funcs[&(func_id, func_index)]
            .generate_func_module(&self.func_to_globals, instantiate_func_global)
    }

    pub fn instantiate_bb(
        &mut self,
        config: JitConfig,
        module_id: ModuleId,
        func_id: FuncId,
        func_index: usize,
        bb_id: BasicBlockId,
        index: usize,
        global_manager: &mut GlobalManager,
        instantiate_func_global: Global,
        closure_global_layout: &mut ClosureGlobalLayout,
        stub_globals: &FxHashMap<usize, Global>,
    ) -> Module {
        let jit_func = self.jit_funcs.get_mut(&(func_id, func_index)).unwrap();
        jit_func.generate_bb_module(
            &config,
            &self.func_to_globals,
            module_id,
            &self.func_types,
            bb_id,
            index,
            &stub_globals,
            closure_global_layout,
            instantiate_func_global,
            global_manager,
        )
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
        closure_global_layout: &mut ClosureGlobalLayout,
    ) -> Self {
        let mut func = func.clone();
        closure_func_assign_types(&mut func, func_index, closure_global_layout);
        // 共通部分式除去を行うと変数の生存期間が伸びてしまい、JITでのパフォーマンスが落ちるのでここでは行わない
        ssa_optimize(&mut func, false);
        let bb_to_globals = func
            .bbs
            .keys()
            .map(|bb_id| (bb_id, global_manager.gen_global(LocalType::FuncRef)))
            .collect::<VecMap<BasicBlockId, _>>();
        let bb_infos = calculate_bb_info(&func);

        // all_typed_objs: BBごとの型推論結果
        // あるBBの型推論結果はその支配集合にまで伝播させる
        let mut all_typed_objs = VecMap::new();
        let predecessors = calc_predecessors(&func.bbs);
        let rpo = calculate_rpo(&func.bbs, func.bb_entry);
        let doms = calc_doms(&func.bbs, &rpo, func.bb_entry, &predecessors);
        let mut rev_doms: FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> = FxHashMap::default();
        for (&bb, dom_set) in &doms {
            for &dom in dom_set {
                rev_doms.entry(dom).or_default().insert(bb);
            }
        }

        for bb_id in func.bbs.keys() {
            all_typed_objs.insert(bb_id, VecMap::new());
        }
        for bb in func.bbs.values() {
            let defs = collect_defs(bb);
            let typed_objs = bb_optimizer::analyze_typed_obj(bb, &defs);
            let dom_set = rev_doms.get(&bb.id).unwrap();
            for &dom_bb_id in dom_set {
                if dom_bb_id == bb.id {
                    // 自分自身のBBで定義されているものは未初期化の可能性があるので伝播しない
                    // TODO: 条件を緩くする
                    continue;
                }
                for (obj, typed_obj) in typed_objs.iter() {
                    all_typed_objs[dom_bb_id].entry(obj).or_insert(*typed_obj);
                }
            }
        }

        /*
        all_typed_objs TODO:
        JITに関係なく行える最適化なのでここに置くべきではない
        // bb0
        if is<int>(x):
            // bb1
            // ここでxはintであると推論するべき

        -----
        // bb0
        unbox<int>(x)
        jmp bb1
        // bb1
        y = move x // yもintであると推論するべき
        */

        let jit_bbs = func
            .bbs
            .values()
            .map(|bb| {
                JitBB {
                    bb_id: bb.id,
                    info: bb_infos[bb.id].clone(),
                    typed_objs: all_typed_objs[bb.id].clone(), // TODO: cloneしないようにする
                    bb_index_manager: BBIndexManager::new(
                        bb_infos[bb.id].type_params.len(),
                        bb_to_globals[bb.id],
                    ),
                }
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
        &self,
        func_to_globals: &TiVec<FuncId, GlobalId>,
        instantiate_func_global: Global,
    ) -> Module {
        let mut funcs = TiVec::<FuncId, _>::new();
        /*
        func entry() {
            set_global f0_ref f0
            set_global bb0_ref [bb0_stub, nil, ..., nil]
            set_global bb1_ref [bb1_stub, nil, ..., nil]
        }
        */
        let entry_func_id = funcs.push_and_get_key(None);
        let body_func_id = funcs.push_and_get_key(None);
        let bb_stub_func_ids = self
            .jit_bbs
            .values()
            .map(|jit_bb| (jit_bb.bb_id, funcs.push_and_get_key(None)))
            .collect::<VecMap<BasicBlockId, _>>();
        let entry_func = {
            let mut locals = VecMap::new();
            let func_ref_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            let mut exprs = Vec::new();
            exprs.extend([
                ExprAssign {
                    local: Some(func_ref_local),
                    expr: Expr::FuncRef(FuncId::from(1)),
                },
                ExprAssign {
                    local: None,
                    expr: Expr::GlobalSet(instantiate_func_global.id, func_ref_local),
                },
            ]);
            if self.func_index == GLOBAL_LAYOUT_DEFAULT_INDEX {
                // func_to_globalsはindex=0のためのもの
                exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::GlobalSet(func_to_globals[self.func.id], func_ref_local),
                });
            }
            for jit_bb in self.jit_bbs.values() {
                let func_ref_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::FuncRef,
                });
                let (_, index_global) = jit_bb
                    .bb_index_manager
                    .from_idx(GLOBAL_LAYOUT_DEFAULT_INDEX);

                exprs.push(ExprAssign {
                    local: Some(func_ref_local),
                    expr: Expr::FuncRef(bb_stub_func_ids[jit_bb.bb_id]),
                });

                exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::GlobalSet(index_global.id, func_ref_local),
                });
            }

            Func {
                id: entry_func_id,
                args: vec![],
                ret_type: LocalType::FuncRef, // TODO: Nilでも返したほうがよさそう
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs,
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(func_ref_local)),
                }]
                .into_iter()
                .collect(),
            }
        };
        funcs[entry_func_id] = Some(entry_func);

        /*
        func f0(...) {
            bb0 <- get_global bb0_ref
            bb0[index](...)
        }
        */
        let entry_bb_info = &self.jit_bbs[self.func.bb_entry].info;
        let body_func = {
            let mut locals = self.func.locals.clone();

            let func_ref_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            let (_, index_global) = self.jit_bbs[self.func.bb_entry]
                .bb_index_manager
                .from_idx(GLOBAL_LAYOUT_DEFAULT_INDEX);

            Func {
                id: body_func_id,
                args: self.func.args.clone(),
                ret_type: self.func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs: vec![ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::GlobalGet(index_global.id),
                    }],
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(
                        ExprCallRef {
                            func: func_ref_local,
                            args: entry_bb_info.args.to_vec(),
                            func_type: FuncType {
                                args: entry_bb_info.arg_types(
                                    &self.func,
                                    &ti_vec![None; entry_bb_info.type_params.len()],
                                ),
                                ret: self.func.ret_type,
                            },
                        },
                    )),
                }]
                .into_iter()
                .collect(),
            }
        };

        funcs[body_func_id] = Some(body_func);

        for jit_bb in self.jit_bbs.values() {
            let func = self.generate_bb_stub_func(
                self.module_id,
                jit_bb,
                bb_stub_func_ids[jit_bb.bb_id],
                GLOBAL_LAYOUT_DEFAULT_INDEX,
            );
            funcs[bb_stub_func_ids[jit_bb.bb_id]] = Some(func);
        }

        Module {
            globals: FxHashMap::default(),
            funcs: funcs.into_iter().map(|f| f.unwrap()).collect(),
            entry: FuncId::from(0),
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }

    fn generate_bb_stub_func(
        &self,
        module_id: ModuleId,
        jit_bb: &JitBB,
        id: FuncId,
        index: usize,
    ) -> Func {
        /*
        func bb0_stub(...) {
            instantiate_bb(..., index)
            bb0 <- get_global bb0_ref
            bb0(...)
        }
        */
        let (type_args, index_global) = jit_bb.bb_index_manager.from_idx(index);
        let mut locals = self.func.locals.clone();
        for (type_param_id, type_arg) in type_args.iter_enumerated() {
            if let Some(typ) = type_arg {
                let local = *jit_bb.info.type_params.get_by_left(&type_param_id).unwrap();
                locals[local].typ = LocalType::Type(Type::Val(*typ));
            }
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
                exprs: vec![
                    ExprAssign {
                        local: None,
                        expr: Expr::InstantiateBB(
                            module_id,
                            self.func.id,
                            self.func_index,
                            jit_bb.bb_id,
                            index,
                        ),
                    },
                    ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::GlobalGet(index_global.id),
                    },
                ],
                next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(ExprCallRef {
                    func: func_ref_local,
                    args: jit_bb.info.args.clone(),
                    func_type: FuncType {
                        args: jit_bb.info.arg_types(&self.func, type_args),
                        ret: self.func.ret_type,
                    },
                })),
            }]
            .into_iter()
            .collect(),
        }
    }

    // TODO: JitBBに移動する
    #[allow(clippy::too_many_arguments)]
    pub fn generate_bb_module(
        &mut self,
        _config: &JitConfig,
        func_to_globals: &TiVec<FuncId, GlobalId>,
        module_id: ModuleId,
        func_types: &VecMap<FuncId, FuncType>,
        orig_entry_bb_id: BasicBlockId,
        index: usize,
        stub_globals: &FxHashMap<usize, Global>,
        closure_global_layout: &mut ClosureGlobalLayout,
        instantiate_func_global: Global,
        global_manager: &mut GlobalManager,
    ) -> Module {
        let mut required_closure_idx = Vec::new();

        {
            // entrypoint_table[0]のスタブはJS APIからも使われるので未初期化の場合作成しておく
            // TODO: generate_stub_moduleで行うべき
            let (closure_idx, flag) = closure_global_layout
                .to_idx(&ClosureArgs::Variadic)
                .unwrap();
            if flag == IndexFlag::NewInstance {
                required_closure_idx.push(closure_idx);
            }
        }

        let mut required_stubs = Vec::new();

        let (type_args, index_global) = self.jit_bbs[orig_entry_bb_id]
            .bb_index_manager
            .from_idx(index);
        let func = &self.func;

        // If/Jump命令で必要なBBの一覧。(新しいモジュールのBB ID, 元のモジュールのBB ID, isで分岐されたときの型情報)のペアのリスト
        let mut required_bbs = Vec::new();

        let mut bbs = VecMap::new();
        let mut funcs = TiVec::new();
        let mut new_locals = func.locals.clone();

        let mut orig_bb_to_new_bb = VecMap::new();
        let mut todo_merge_bb_ids = vec![orig_entry_bb_id];
        let bb_entry = bbs.allocate_key();
        orig_bb_to_new_bb.insert(orig_entry_bb_id, bb_entry);

        let mut assigned_local_to_obj = FxBiHashMap::default();

        // 型代入後のDefUseChain
        let mut def_use_chain = DefUseChain::new();
        while let Some(orig_bb_id) = todo_merge_bb_ids.pop() {
            let new_bb_id = orig_bb_to_new_bb[orig_bb_id];

            let orig_next = &func.bbs[orig_bb_id].next;
            let mut bb = func.bbs[orig_bb_id].clone();
            bb.id = new_bb_id;

            if orig_bb_id == orig_entry_bb_id {
                assigned_local_to_obj = bb_optimizer::assign_type_args(
                    &mut new_locals,
                    &mut bb,
                    &self.jit_bbs[orig_entry_bb_id].info.type_params,
                    type_args,
                );
            } else {
                for (local, _) in bb.local_usages_mut() {
                    if let Some(&obj_local) = assigned_local_to_obj.get_by_left(local) {
                        *local = obj_local;
                    }
                }
            }
            let mut typed_objs = self.jit_bbs[orig_bb_id].typed_objs.clone();
            for (local, obj_local) in &assigned_local_to_obj {
                typed_objs.insert(
                    *obj_local,
                    TypedObj {
                        val_type: *local,
                        typ: match new_locals[*local].typ {
                            LocalType::Type(Type::Val(v)) => v,
                            _ => unreachable!("obj_local must be Val type"),
                        },
                    },
                );
            }

            /*
            TODO: bbsが完成した後に行う
            if config.enable_optimization {
                bb_optimizer::remove_type_check(&mut bb, &typed_objs, &defs);
                // bb_optimizer::assign_type_argsの結果出来たto_obj/from_objの除去が主な目的
                bb_optimizer::copy_propagate(&new_locals, &mut bb);
            }*/

            // Args専用のuse-def chainのようなもの
            let mut local_to_args_expr_idx = FxHashMap::default();
            let mut exprs = Vec::new();
            for expr in bb.exprs.iter() {
                // FuncRefとCall命令はget global命令に置き換えられる
                match *expr {
                    ExprAssign {
                        local,
                        expr: Expr::Phi(ref incomings),
                    } => {
                        if orig_bb_id == orig_entry_bb_id {
                            // 削除
                        } else {
                            let incomings = incomings
                                .iter()
                                .filter_map(|incoming| {
                                    // orig_bb_to_new_bbに存在しないものは消していい気がするので消しているが自信がない
                                    // 例えば if (true) { bb1 } else { bb2 } phi(local1 from bb1, local2 from bb2) みたいな場合、存在しなくなるはず
                                    orig_bb_to_new_bb.get(incoming.bb).copied().map(|bb| {
                                        PhiIncomingValue {
                                            bb,
                                            local: incoming.local,
                                        }
                                    })
                                })
                                .collect();
                            exprs.push(ExprAssign {
                                local,
                                expr: Expr::Phi(incomings),
                            });
                        }
                    }
                    ExprAssign {
                        local,
                        expr: Expr::FuncRef(id),
                    } => {
                        exprs.push(ExprAssign {
                            local,
                            expr: Expr::GlobalGet(func_to_globals[id]),
                        });
                    }
                    ExprAssign {
                        local,
                        expr: Expr::Call(ExprCall { func_id, ref args }),
                    } => {
                        let func_ref_local = new_locals.push_with(|id| Local {
                            id,
                            typ: LocalType::FuncRef,
                        });

                        exprs.push(ExprAssign {
                            local: Some(func_ref_local),
                            expr: Expr::GlobalGet(func_to_globals[func_id]),
                        });
                        exprs.push(ExprAssign {
                            local,
                            expr: Expr::CallRef(ExprCallRef {
                                func: func_ref_local,
                                args: args.clone(),
                                func_type: func_types[func_id].clone(),
                            }),
                        });
                    }
                    ExprAssign {
                        local,
                        expr: Expr::EntrypointTable(_),
                    } => {
                        let mut locals = Vec::new();
                        for index in 0..GLOBAL_LAYOUT_MAX_SIZE {
                            let stub = new_locals.push_with(|id| Local {
                                id,
                                typ: LocalType::MutFuncRef,
                            });
                            exprs.push(ExprAssign {
                                local: Some(stub),
                                expr: Expr::GlobalGet(stub_globals[&index].id),
                            });
                            locals.push(stub);
                        }
                        exprs.push(ExprAssign {
                            local,
                            expr: Expr::EntrypointTable(locals),
                        });
                    }
                    ExprAssign {
                        local,
                        expr: Expr::CallClosure(ref call_closure),
                    } => {
                        let call_closure = Self::specialize_call_closure(
                            call_closure,
                            &exprs,
                            closure_global_layout,
                            &local_to_args_expr_idx,
                            &mut required_closure_idx,
                            &typed_objs,
                        )
                        .unwrap_or_else(|| call_closure.clone());

                        exprs.push(ExprAssign {
                            local,
                            expr: Expr::CallClosure(call_closure),
                        });
                    }
                    ExprAssign {
                        local: Some(local),
                        expr: Expr::VariadicArgs(_),
                    } => {
                        local_to_args_expr_idx.insert(local, exprs.len());
                        exprs.push(expr.clone());
                    }
                    ExprAssign {
                        local: Some(local),
                        expr: Expr::Move(value),
                    } if let Some(args_expr_idx) = local_to_args_expr_idx.get(&value) => {
                        local_to_args_expr_idx.insert(local, *args_expr_idx);
                        exprs.push(expr.clone());
                    }
                    ref expr => {
                        exprs.push(expr.clone());
                    }
                }
            }
            bb.exprs = exprs;

            // nextの決定にdef_use_chainとbbsが必要なので、一旦計算し、bbsに追加する
            // bb.nextはdef_use_chainの計算には影響を与えないのでここで計算して問題ない
            def_use_chain.add_bb(&bb);
            bbs.insert_node(bb);

            // nextがtail callならexpr::callと同じようにget globalに置き換える
            // nextがif/jumpなら、BBに対応する関数へのジャンプに置き換える
            let next = match *orig_next {
                BasicBlockNext::If(cond, orig_then_bb_id, orig_else_bb_id) => {
                    let cond_expr = def_use_chain.get_def_non_move_expr(&bbs, cond);
                    // もし cond が Is<T>(obj) かつ、obj が to_obj<P> ならば分岐をなくす
                    // この形の定数畳み込みのみ assign_type_args で新たに生まれるためここで処理する
                    // それ以外の形の定数畳み込みはJITとは無関係に外部で行う
                    let const_cond = if let Some(&Expr::Is(ty1, obj)) = cond_expr
                        && let Some(&Expr::ToObj(ty2, _)) =
                            def_use_chain.get_def_non_move_expr(&bbs, obj)
                    {
                        // TODO: 定数畳み込みを実装したのでもうなくていいかも
                        Some(ty1 == ty2)
                    } else if let Some(&Expr::Bool(b)) = cond_expr {
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
                        let next_bb_id =
                            if let Some(&next_bb_id) = orig_bb_to_new_bb.get(orig_next_bb_id) {
                                next_bb_id
                            } else {
                                let next_bb_id = bbs.allocate_key();
                                orig_bb_to_new_bb.insert(orig_next_bb_id, next_bb_id);
                                todo_merge_bb_ids.push(orig_next_bb_id);
                                next_bb_id
                            };
                        BasicBlockNext::Jump(next_bb_id)
                    } else {
                        let mut then_types = Vec::new();
                        // Is命令で分岐している場合、then側のBBで型情報を使える
                        if let Some(&Expr::Is(typ, obj_local)) = cond_expr {
                            then_types.push((obj_local, typ));
                        }

                        let then_bb_id = bbs.allocate_key();
                        let else_bb_id = bbs.allocate_key();
                        required_bbs.push((then_bb_id, orig_then_bb_id, then_types));
                        required_bbs.push((else_bb_id, orig_else_bb_id, Vec::new()));

                        BasicBlockNext::If(cond, then_bb_id, else_bb_id)
                    }
                }
                BasicBlockNext::Jump(orig_next_bb_id) => {
                    let next_bb_id =
                        if let Some(&next_bb_id) = orig_bb_to_new_bb.get(orig_next_bb_id) {
                            next_bb_id
                        } else {
                            let next_bb_id = bbs.allocate_key();
                            orig_bb_to_new_bb.insert(orig_next_bb_id, next_bb_id);
                            todo_merge_bb_ids.push(orig_next_bb_id);
                            next_bb_id
                        };

                    BasicBlockNext::Jump(next_bb_id)
                }
                BasicBlockNext::Terminator(BasicBlockTerminator::TailCall(ExprCall {
                    func_id,
                    ref args,
                })) => {
                    let func_ref_local = new_locals.push_with(|id| Local {
                        id,
                        typ: LocalType::FuncRef,
                    });
                    let exprs = &mut bbs[new_bb_id].exprs;
                    exprs.push(ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::GlobalGet(func_to_globals[func_id]),
                    });
                    BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(ExprCallRef {
                        func: func_ref_local,
                        args: args.clone(),
                        func_type: func_types[func_id].clone(),
                    }))
                }
                BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(
                    ref call_closure,
                )) => {
                    let call_closure = Self::specialize_call_closure(
                        call_closure,
                        &bbs[new_bb_id].exprs,
                        closure_global_layout,
                        &local_to_args_expr_idx,
                        &mut required_closure_idx,
                        &typed_objs,
                    )
                    .unwrap_or_else(|| call_closure.clone());
                    BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(call_closure))
                }
                ref next @ BasicBlockNext::Terminator(
                    BasicBlockTerminator::TailCallRef(_)
                    | BasicBlockTerminator::Return(_)
                    | BasicBlockTerminator::Error(_),
                ) => {
                    let mut new_next = next.clone();
                    for local in new_next.local_ids_mut() {
                        if let Some(&obj_local) = assigned_local_to_obj.get_by_left(local) {
                            *local = obj_local;
                        }
                    }
                    new_next
                }
            };

            bbs[new_bb_id].next = next;
            def_use_chain.add_bb(&bbs[new_bb_id]); // nextでexprsが変わった可能性があるので更新
        }

        for (bb_id, orig_bb_id, types) in required_bbs {
            let mut exprs = Vec::new();
            // ジャンプ先のBBのPhiはここに移動
            // TODO: 型代入を考慮
            for expr_assign in &func.bbs[orig_bb_id].exprs {
                if let ExprAssign {
                    local,
                    expr: Expr::Phi(incomings),
                } = expr_assign
                {
                    let incomings = incomings
                        .iter()
                        .map(|incoming| PhiIncomingValue {
                            bb: orig_bb_to_new_bb[incoming.bb],
                            local: incoming.local,
                        })
                        .collect();
                    exprs.push(ExprAssign {
                        local: *local,
                        expr: Expr::Phi(incomings),
                    });
                }
            }

            // この分岐でのみ成り立つtyped obj
            let mut branch_typed_objs = VecMap::new();
            for (obj_local, typ) in types {
                let val_local = new_locals.push_with(|id| Local {
                    id,
                    typ: LocalType::Type(Type::Val(typ)),
                });
                exprs.push(ExprAssign {
                    local: Some(val_local),
                    expr: Expr::FromObj(typ, obj_local),
                });
                branch_typed_objs.insert(
                    obj_local,
                    TypedObj {
                        val_type: val_local,
                        typ,
                    },
                );
            }
            // ここでのtyped_objsは事前計算で分かるもの or この分岐でのみ成り立つもの or assigned_local_to_obj によって追加されたto_objのいずれかである
            let callee_jit_bb = &mut self.jit_bbs[orig_bb_id];
            let (locals_to_pass, type_args, index_global) = calculate_args_to_pass(
                &callee_jit_bb.info,
                |obj_local| {
                    // TODO: typed_objs と branch_typed_objsのマージでいいかも
                    callee_jit_bb
                        .typed_objs
                        .get(obj_local)
                        .copied()
                        .or_else(|| branch_typed_objs.get(obj_local).copied())
                        .or_else(|| {
                            def_use_chain
                                .get_def_non_move_expr(&bbs, obj_local)
                                .and_then(|expr| {
                                    if let Expr::ToObj(typ, val_local) = *expr {
                                        Some(TypedObj {
                                            val_type: val_local,
                                            typ,
                                        })
                                    } else {
                                        None
                                    }
                                })
                        })
                },
                &assigned_local_to_obj,
                &mut callee_jit_bb.bb_index_manager,
                &mut required_stubs,
                global_manager,
            );

            let func_ref_local = new_locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            exprs.extend([ExprAssign {
                local: Some(func_ref_local),
                expr: Expr::GlobalGet(index_global.id),
            }]);

            bbs.insert_node(BasicBlock {
                id: bb_id,
                exprs,
                next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(ExprCallRef {
                    func: func_ref_local,
                    args: locals_to_pass,
                    func_type: FuncType {
                        args: self.jit_bbs[orig_bb_id].info.arg_types(func, &type_args),
                        ret: func.ret_type,
                    },
                })),
            });
        }

        let body_func = Func {
            id: funcs.next_key(),
            args: self.jit_bbs[orig_entry_bb_id].info.args.clone(),
            ret_type: func.ret_type,
            locals: new_locals,
            bb_entry,
            bbs,
        };

        let body_func_id = body_func.id;
        funcs.push(body_func);

        let required_stubs = required_stubs
            .iter()
            .filter(|(_, _, index)| *index != GLOBAL_LAYOUT_DEFAULT_INDEX)
            .map(|&(bb_id, index_global, index)| {
                let bb_stub_func_id = funcs.next_key();
                let func = self.generate_bb_stub_func(
                    module_id,
                    &self.jit_bbs[bb_id],
                    bb_stub_func_id,
                    index,
                );
                funcs.push(func);
                (bb_id, index_global, bb_stub_func_id)
            })
            .collect::<Vec<_>>();

        let required_closure_idx = required_closure_idx
            .iter()
            .map(|&closure_idx| {
                let stub_func_id = funcs.next_key();

                let mut locals = VecMap::new();
                let mut args = Vec::new();
                let closure_local = locals.push_with(|id| Local {
                    id,
                    typ: ValType::Closure.into(),
                });
                let mut arg_locals = Vec::new();
                args.push(closure_local);

                match closure_global_layout.from_idx(closure_idx) {
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

                exprs.push(ExprAssign {
                    local: Some(module_id_local),
                    expr: Expr::ClosureModuleId(closure_local),
                });
                exprs.push(ExprAssign {
                    local: Some(func_id_local),
                    expr: Expr::ClosureFuncId(closure_local),
                });
                exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::InstantiateClosureFunc(module_id_local, func_id_local, closure_idx),
                });
                exprs.push(ExprAssign {
                    local: Some(func_ref_local),
                    expr: Expr::GlobalGet(instantiate_func_global.id),
                });
                exprs.push(ExprAssign {
                    local: Some(mut_func_ref_local),
                    expr: Expr::CreateMutFuncRef(func_ref_local),
                });
                exprs.push(ExprAssign {
                    local: Some(entrypoint_table_local),
                    expr: Expr::ClosureEntrypointTable(closure_local),
                });
                exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::SetEntrypointTable(
                        closure_idx,
                        entrypoint_table_local,
                        mut_func_ref_local,
                    ),
                });

                let arg_types = arg_locals.iter().map(|&local| locals[local].typ).collect();

                funcs.push(Func {
                    id: stub_func_id,
                    args,
                    ret_type: LocalType::Type(Type::Obj),
                    locals,
                    bb_entry: BasicBlockId::from(0),
                    bbs: [BasicBlock {
                        id: BasicBlockId::from(0),
                        exprs,
                        next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallClosure(
                            ExprCallClosure {
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

        let entry_func = {
            let mut locals = VecMap::new();

            let mut bbs = VecMap::new();

            let func_ref_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            bbs.insert_node({
                let mut exprs = vec![
                    ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::FuncRef(body_func_id),
                    },
                    ExprAssign {
                        local: None,
                        expr: Expr::GlobalSet(index_global.id, func_ref_local),
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
                    exprs.push(ExprAssign {
                        local: Some(stub_func_ref_local),
                        expr: Expr::FuncRef(stub_func_id),
                    });
                    exprs.push(ExprAssign {
                        local: Some(stub_mut_func_ref_local),
                        expr: Expr::GlobalGet(stub_globals[&closure_idx].id),
                    });
                    exprs.push(ExprAssign {
                        local: None,
                        expr: Expr::SetMutFuncRef(stub_mut_func_ref_local, stub_func_ref_local),
                    });
                }

                /*
                bbX_ref = stub_func
                */

                for &(_, index_global, stub_func_id) in required_stubs.iter() {
                    let func_ref_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::FuncRef,
                    });

                    exprs.extend([
                        ExprAssign {
                            local: Some(func_ref_local),
                            expr: Expr::FuncRef(stub_func_id),
                        },
                        ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(index_global.id, func_ref_local),
                        },
                    ]);
                }

                BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs,
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(func_ref_local)),
                }
            });

            Func {
                id: funcs.next_key(),
                args: vec![],
                ret_type: LocalType::FuncRef, // TODO: Nilでも返したほうがよさそう
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs,
            }
        };
        let entry_func_id = entry_func.id;
        funcs.push(entry_func);

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

    fn specialize_call_closure(
        call_closure: &ExprCallClosure,
        exprs: &[ExprAssign],
        closure_global_layout: &mut ClosureGlobalLayout,
        local_to_args_expr_idx: &FxHashMap<LocalId, usize>,
        required_closure_idx: &mut Vec<usize>,
        typed_objs: &VecMap<LocalId, TypedObj>,
    ) -> Option<ExprCallClosure> {
        if call_closure.func_index != GLOBAL_LAYOUT_DEFAULT_INDEX {
            return None;
        }

        // func_index == GLOBAL_LAYOUT_DEFAULT_INDEX なら引数は[Args]を仮定してよい
        let args_expr_idx = *local_to_args_expr_idx.get(&call_closure.args[0])?;
        let Expr::VariadicArgs(args) = &exprs[args_expr_idx].expr else {
            unreachable!("unexpected expr other than VariadicArgs");
        };

        let mut fixed_args = Vec::new();
        let mut fixed_arg_types = Vec::new();
        for &obj_arg in args {
            if let Some(typed_obj) = typed_objs.get(obj_arg) {
                fixed_args.push(typed_obj.val_type);
                fixed_arg_types.push(Type::from(typed_obj.typ));
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
            .to_idx(&ClosureArgs::Specified(fixed_arg_types))
            .unwrap_or_else(|| {
                closure_global_layout
                    .to_idx(&ClosureArgs::Variadic)
                    .unwrap()
            });

        if flag == IndexFlag::NewInstance {
            required_closure_idx.push(closure_index);
        }
        Some(if closure_index == GLOBAL_LAYOUT_DEFAULT_INDEX {
            call_closure.clone()
        } else {
            ExprCallClosure {
                closure: call_closure.closure,
                args: fixed_args,
                arg_types,
                func_index: closure_index,
            }
        })
    }
}

#[derive(Debug)]
struct JitBB {
    bb_id: BasicBlockId,
    info: BBInfo,
    typed_objs: VecMap<LocalId, TypedObj>,
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
    fn arg_types(
        &self,
        func: &Func,
        type_args: &TiVec<TypeParamId, Option<ValType>>,
    ) -> Vec<LocalType> {
        self.args
            .iter()
            .map(|&arg| {
                if let Some(&type_param_id) = self.type_params.get_by_right(&arg)
                    && let Some(typ) = type_args.get(type_param_id).copied().flatten()
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

        let mut type_params = TiVec::new();
        for &arg in &args {
            if let LocalType::Type(Type::Obj) = func.locals[arg].typ {
                type_params.push(arg);
            }
        }

        let info = BBInfo {
            bb_id,
            args,
            type_params: type_params.into_iter_enumerated().collect(),
        };

        bb_info.insert_node(info);
    }
    bb_info
}

fn calculate_args_to_pass(
    callee: &BBInfo,
    caller_typed_objs: impl Fn(LocalId) -> Option<TypedObj>,
    caller_assigned_local_to_obj: &FxBiHashMap<LocalId, LocalId>,
    bb_index_manager: &mut BBIndexManager,
    required_stubs: &mut Vec<(BasicBlockId, Global, usize)>,
    global_manager: &mut GlobalManager,
) -> (Vec<LocalId>, TiVec<TypeParamId, Option<ValType>>, Global) {
    let mut type_args = ti_vec![None; callee.type_params.len()];
    let mut args_to_pass = Vec::new();

    for &arg in &callee.args {
        let obj_arg = caller_assigned_local_to_obj
            .get_by_left(&arg)
            .copied()
            .unwrap_or(arg);

        let caller_args = if let Some(&type_param_id) = callee.type_params.get_by_right(&arg)
            && let Some(caller_typed_obj) = caller_typed_objs(obj_arg)
        {
            type_args[type_param_id] = Some(caller_typed_obj.typ);
            caller_typed_obj.val_type
        } else {
            obj_arg
        };

        args_to_pass.push(caller_args);
    }

    if let Some((global, index, flag)) = bb_index_manager.to_idx(&type_args, global_manager) {
        if flag == IndexFlag::NewInstance {
            required_stubs.push((callee.bb_id, global, index));
        }
        (args_to_pass, type_args, global)
    } else {
        // `|_| None` を渡すと "reached the recursion limit while instantiating" が発生するため
        fn empty_typed_objs(_: LocalId) -> Option<TypedObj> {
            None
        }

        // global layoutが満杯なら型パラメータなしで再計算
        // 型パラメータなしで呼び出すとto_idxの結果は必ずSomeになるので無限ループすることはない
        calculate_args_to_pass(
            callee,
            empty_typed_objs,
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
    type_params_to_index: FxBiHashMap<TiVec<TypeParamId, Option<ValType>>, usize>,
    index_to_global: FxHashMap<usize, Global>,
}

impl BBIndexManager {
    pub fn new(params_len: usize, global: Global) -> Self {
        let mut type_params_to_index = FxBiHashMap::default();
        let mut index_to_global = FxHashMap::default();
        type_params_to_index.insert(ti_vec![None; params_len], GLOBAL_LAYOUT_DEFAULT_INDEX);
        index_to_global.insert(GLOBAL_LAYOUT_DEFAULT_INDEX, global);
        Self {
            type_params_to_index,
            index_to_global,
        }
    }

    pub fn to_idx(
        &mut self,
        type_params: &TiVec<TypeParamId, Option<ValType>>,
        global_manager: &mut GlobalManager,
    ) -> Option<(Global, usize, IndexFlag)> {
        if let Some(&index) = self.type_params_to_index.get_by_left(type_params) {
            let global = *self.index_to_global.get(&index).unwrap();
            Some((global.to_import(), index, IndexFlag::ExistingInstance))
        } else if self.type_params_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.type_params_to_index.len();
            self.type_params_to_index.insert(type_params.clone(), index);
            let global = global_manager.gen_global(LocalType::FuncRef);
            self.index_to_global.insert(index, global);
            Some((global, index, IndexFlag::NewInstance))
        } else {
            None
        }
    }

    pub fn from_idx(&self, index: usize) -> (&TiVec<TypeParamId, Option<ValType>>, Global) {
        (
            self.type_params_to_index.get_by_right(&index).unwrap(),
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

    pub fn to_idx(&mut self, args: &ClosureArgs) -> Option<(usize, IndexFlag)> {
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

    pub fn from_idx(&self, index: usize) -> &ClosureArgs {
        self.args_to_index.get_by_right(&index).unwrap()
    }
}

fn closure_func_assign_types(
    func: &mut Func,
    func_index: usize,
    closure_global_layout: &ClosureGlobalLayout,
) {
    let ClosureArgs::Specified(args_type) = closure_global_layout.from_idx(func_index) else {
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
                exprs.push(ExprAssign {
                    local: Some(obj_local),
                    expr: Expr::ToObj(val_type, local),
                });
                obj_local
            } else {
                local
            };
            obj_locals.push(obj_local);
        }

        exprs.push(ExprAssign {
            local: Some(variadic_args_local),
            expr: Expr::VariadicArgs(obj_locals),
        });

        BasicBlock {
            id: bb_id,
            exprs,
            next: BasicBlockNext::Jump(prev_entry),
        }
    });

    func.args = new_args;
    func.bb_entry = new_bb_entry;
}
