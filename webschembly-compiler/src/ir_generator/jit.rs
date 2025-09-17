use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::{TiVec, ti_vec};

use super::bb_optimizer;
use crate::ir::*;
use crate::ir_generator::GlobalManager;

#[derive(Debug)]
pub struct Jit {
    jit_module: TiVec<ModuleId, JitModule>,
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}

impl Jit {
    pub fn new() -> Self {
        Self {
            jit_module: TiVec::new(),
        }
    }

    pub fn register_module(
        &mut self,
        global_manager: &mut GlobalManager,
        module: Module,
    ) -> Module {
        let module_id = self.jit_module.next_key();
        self.jit_module
            .push(JitModule::new(global_manager, module_id, module));
        self.jit_module[module_id].generate_stub_module()
    }

    pub fn instantiate_func(
        &mut self,
        global_manager: &mut GlobalManager,
        module_id: ModuleId,
        func_id: FuncId,
    ) -> Module {
        let jit_func = self.jit_module[module_id].generate_jit_func(global_manager, func_id);
        self.jit_module[module_id].jit_funcs[func_id] = Some(jit_func);

        self.jit_module[module_id].jit_funcs[func_id]
            .as_ref()
            .unwrap()
            .generate_stub_module(&self.jit_module[module_id])
    }

    pub fn instantiate_bb(
        &self,
        module_id: ModuleId,
        func_id: FuncId,
        bb_id: BasicBlockId,
    ) -> Module {
        let jit_module = &self.jit_module[module_id];
        let jit_func = self.jit_module[module_id].jit_funcs[func_id]
            .as_ref()
            .unwrap();
        jit_func.generate_bb_module(jit_module, bb_id)
    }
}

#[derive(Debug)]
struct JitModule {
    module_id: ModuleId,
    module: Module,
    jit_funcs: TiVec<FuncId, Option<JitFunc>>,
    func_to_globals: TiVec<FuncId, GlobalId>,
    globals: FxHashSet<GlobalId>,
}

impl JitModule {
    fn new(global_manager: &mut GlobalManager, module_id: ModuleId, module: Module) -> Self {
        let jit_funcs = (0..module.funcs.len()).map(|_| None).collect();

        let func_to_globals = module
            .funcs
            .iter()
            .map(|_| global_manager.gen_global_id())
            .collect::<TiVec<FuncId, _>>();

        let globals = {
            let mut globals = module.globals.clone();
            globals.extend(func_to_globals.iter());
            globals
        };

        Self {
            module_id,
            module,
            jit_funcs,
            func_to_globals,
            globals,
        }
    }

    fn generate_stub_module(&self) -> Module {
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

        let func = Func {
            id: funcs.next_key(),
            args: 0,
            ret_type: LocalType::Type(Type::Boxed),
            locals: ti_vec![
                LocalType::Type(Type::Boxed),
                LocalType::Type(Type::Val(ValType::FuncRef)),
                LocalType::Type(Type::Boxed),
            ],
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![BasicBlock {
                id: BasicBlockId::from(0),
                exprs: {
                    let mut exprs = Vec::new();
                    exprs.push(ExprAssign {
                        local: None,
                        expr: Expr::InitModule,
                    });
                    for func in self.module.funcs.iter() {
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::FuncRef(stub_func_ids[func.id]),
                        });
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(2)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(1)),
                        });
                        exprs.push(ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(self.func_to_globals[func.id], LocalId::from(2)),
                        });
                    }
                    exprs
                },
                next: BasicBlockNext::TailCall(ExprCall {
                    func_id: stub_func_ids[self.module.entry],
                    args: vec![],
                })
            },],
            jit_strategy: FuncJitStrategy::Never,
        };
        funcs.push(func);
        for func in self.module.funcs.iter() {
            /*
            以下のようなスタブを生成
            func f0_stub(x1, x2) {
                if f0_ref == f0_stub
                    instantiate_module(f0_module);
                f0 <- get_global f0_ref
                f0(x1, x2)
            }
                            */
            let func = Func {
                id: funcs.next_key(),
                args: func.args,
                ret_type: func.ret_type,
                locals: {
                    let mut locals = TiVec::new();
                    locals.extend(func.arg_types().into_iter());
                    locals.extend(vec![
                        func.ret_type(),
                        LocalType::Type(Type::Boxed), // boxed f0_ref
                        LocalType::Type(Type::Val(ValType::FuncRef)), // f0_ref
                        LocalType::Type(Type::Boxed), // f0_stub
                        LocalType::Type(Type::Val(ValType::Bool)), // f0_ref != f0_stub
                    ]);
                    locals
                },
                bb_entry: BasicBlockId::from(0),
                bbs: ti_vec![
                    BasicBlock {
                        id: BasicBlockId::from(0),
                        exprs: vec![
                            ExprAssign {
                                local: Some(LocalId::from(func.args + 1)),
                                expr: Expr::GlobalGet(self.func_to_globals[func.id]),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(func.args + 3)),
                                expr: Expr::FuncRef(stub_func_ids[func.id]),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(func.args + 4)),
                                expr: Expr::Eq(
                                    LocalId::from(func.args + 1),
                                    LocalId::from(func.args + 3),
                                ),
                            },
                        ],
                        next: BasicBlockNext::If(
                            LocalId::from(func.args + 4),
                            BasicBlockId::from(1),
                            BasicBlockId::from(2),
                        ),
                    },
                    BasicBlock {
                        id: BasicBlockId::from(1),
                        exprs: vec![ExprAssign {
                            local: None,
                            expr: Expr::InstantiateFunc(self.module_id, func.id),
                        }],
                        next: BasicBlockNext::Jump(BasicBlockId::from(2)),
                    },
                    BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![
                            ExprAssign {
                                local: Some(LocalId::from(func.args + 1)),
                                expr: Expr::GlobalGet(self.func_to_globals[func.id]),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(func.args + 2)),
                                expr: Expr::Unbox(ValType::FuncRef, LocalId::from(func.args + 1)),
                            },
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: LocalId::from(func.args + 2),
                            args: (0..func.args).map(LocalId::from).collect::<Vec<_>>(),
                            func_type: func.func_type()
                        })
                    },
                ],
                jit_strategy: FuncJitStrategy::Never,
            };
            funcs.push(func);
        }

        Module {
            globals: self.globals.clone(),
            funcs,
            entry: FuncId::from(0),
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }

    fn generate_jit_func(&self, global_manager: &mut GlobalManager, func_id: FuncId) -> JitFunc {
        JitFunc::new(global_manager, self, func_id)
    }
}

#[derive(Debug)]
struct JitFunc {
    func_id: FuncId,
    jit_bbs: TiVec<BasicBlockId, JitBB>,
    globals: FxHashSet<GlobalId>,
}

impl JitFunc {
    fn new(global_manager: &mut GlobalManager, jit_module: &JitModule, func_id: FuncId) -> Self {
        let module = &jit_module.module;
        let func = &module.funcs[func_id];
        let bb_to_globals = func
            .bbs
            .iter()
            .map(|_| global_manager.gen_global_id())
            .collect::<TiVec<BasicBlockId, _>>();

        let bb_infos = calculate_bb_info(&func.locals, analyze_locals(func));

        let globals = {
            let mut globals = jit_module.globals.clone();
            globals.extend(bb_to_globals.iter());
            globals
        };

        Self {
            func_id,
            jit_bbs: func
                .bbs
                .iter()
                .map(|bb| JitBB {
                    bb_id: bb.id,
                    global: bb_to_globals[bb.id],
                    info: bb_infos[bb.id].clone(),
                })
                .collect::<TiVec<BasicBlockId, _>>(),
            globals,
        }
    }

    fn generate_stub_module(&self, jit_module: &JitModule) -> Module {
        let module = &jit_module.module;
        let func = &module.funcs[self.func_id];

        let mut funcs = TiVec::<FuncId, _>::new();
        /*
        func entry() {
            set_global f0_ref f0
            set_global bb0_ref bb0_stub
            set_global bb1_ref bb1_stub
        }
        */
        let entry_func = Func {
            id: funcs.next_key(),
            args: 0,
            ret_type: LocalType::Type(Type::Val(ValType::FuncRef)), // TODO: Nilでも返したほうがよさそう
            locals: ti_vec![
                LocalType::Type(Type::Val(ValType::FuncRef)),
                LocalType::Type(Type::Boxed)
            ],
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![BasicBlock {
                id: BasicBlockId::from(0),
                exprs: {
                    let mut exprs = Vec::new();
                    exprs.extend([
                        ExprAssign {
                            local: None,
                            expr: Expr::InitModule,
                        },
                        ExprAssign {
                            local: Some(LocalId::from(0)),
                            expr: Expr::FuncRef(FuncId::from(1)),
                        },
                        ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(0)),
                        },
                        ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(
                                jit_module.func_to_globals[func.id],
                                LocalId::from(1),
                            ),
                        },
                    ]);
                    for jit_bb in self.jit_bbs.iter() {
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(0)),
                            expr: Expr::FuncRef(FuncId::from(2 + usize::from(jit_bb.bb_id))),
                        });
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(0)),
                        });
                        exprs.push(ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(jit_bb.global, LocalId::from(1)),
                        });
                    }
                    exprs
                },
                next: BasicBlockNext::Return(LocalId::from(0)),
            },],
            jit_strategy: FuncJitStrategy::Never,
        };
        funcs.push(entry_func);

        /*
        func f0(...) {
            bb0 <- get_global bb0_ref
            bb0(...)
        }
        */
        let entry_bb_info = &self.jit_bbs[func.bb_entry].info;
        let body_func = Func {
            id: funcs.next_key(),
            args: func.args,
            ret_type: func.ret_type,
            locals: {
                let mut locals = TiVec::new();
                locals.extend(func.arg_types());
                locals.push(LocalType::Type(Type::Boxed)); // boxed bb0_ref
                locals.push(LocalType::Type(Type::Val(ValType::FuncRef))); // bb0_ref
                locals
            },
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![BasicBlock {
                id: BasicBlockId::from(0),
                exprs: vec![
                    ExprAssign {
                        local: Some(LocalId::from(func.args)),
                        expr: Expr::GlobalGet(self.jit_bbs[func.bb_entry].global),
                    },
                    ExprAssign {
                        local: Some(LocalId::from(func.args + 1)),
                        expr: Expr::Unbox(ValType::FuncRef, LocalId::from(func.args),),
                    },
                ],
                next: BasicBlockNext::TailCallRef(ExprCallRef {
                    func: LocalId::from(func.args + 1),
                    args: entry_bb_info
                        .args
                        .iter()
                        .map(|&arg| entry_bb_info.to_original_locals_mapping[arg])
                        .collect::<Vec<_>>(),
                    func_type: FuncType {
                        args: entry_bb_info.arg_types(func),
                        ret: func.ret_type,
                    },
                })
            },],
            jit_strategy: FuncJitStrategy::BasicBlock,
        };

        funcs.push(body_func);

        for (bb_id, jit_bb) in self.jit_bbs.iter_enumerated() {
            /*
            func bb0_stub(...) {
                if bb0_ref == bb0_stub
                    instantiate_bb(...)
                bb0 <- get_global bb0_ref
                bb0(...)
            }
            */
            let mut locals = TiVec::new();
            locals.extend(jit_bb.info.arg_types(func));
            locals.extend([
                func.ret_type(),
                LocalType::Type(Type::Boxed), // boxed bb0_ref
                LocalType::Type(Type::Val(ValType::FuncRef)), // bb0_ref
                LocalType::Type(Type::Boxed), // bb0_stub
                LocalType::Type(Type::Val(ValType::Bool)), // bb0_ref != bb0_stub
            ]);

            let func = Func {
                id: funcs.next_key(),
                args: jit_bb.info.args.len(),
                ret_type: func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: ti_vec![
                    BasicBlock {
                        id: BasicBlockId::from(0),
                        exprs: vec![
                            ExprAssign {
                                local: Some(LocalId::from(jit_bb.info.args.len() + 1)),
                                expr: Expr::GlobalGet(jit_bb.global),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(jit_bb.info.args.len() + 3)),
                                expr: Expr::FuncRef(FuncId::from(2 + usize::from(bb_id))),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(jit_bb.info.args.len() + 4)),
                                expr: Expr::Eq(
                                    LocalId::from(jit_bb.info.args.len() + 1),
                                    LocalId::from(jit_bb.info.args.len() + 3),
                                ),
                            },
                        ],
                        next: BasicBlockNext::If(
                            LocalId::from(jit_bb.info.args.len() + 4),
                            BasicBlockId::from(1),
                            BasicBlockId::from(2),
                        ),
                    },
                    BasicBlock {
                        id: BasicBlockId::from(1),
                        exprs: vec![ExprAssign {
                            local: None,
                            expr: Expr::InstantiateBB(jit_module.module_id, func.id, bb_id),
                        }],
                        next: BasicBlockNext::Jump(BasicBlockId::from(2)),
                    },
                    BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![
                            ExprAssign {
                                local: Some(LocalId::from(jit_bb.info.args.len() + 1)),
                                expr: Expr::GlobalGet(jit_bb.global),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(jit_bb.info.args.len() + 2)),
                                expr: Expr::Unbox(
                                    ValType::FuncRef,
                                    LocalId::from(jit_bb.info.args.len() + 1),
                                ),
                            }
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: LocalId::from(jit_bb.info.args.len() + 2),
                            args: (0..jit_bb.info.args.len())
                                .map(LocalId::from)
                                .collect::<Vec<_>>(),
                            func_type: FuncType {
                                args: jit_bb.info.arg_types(func),
                                ret: func.ret_type,
                            },
                        }),
                    },
                ],
                jit_strategy: FuncJitStrategy::Never,
            };
            funcs.push(func);
        }

        Module {
            globals: self.globals.clone(),
            funcs,
            entry: FuncId::from(0),
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }

    fn generate_bb_module(&self, jit_module: &JitModule, bb_id: BasicBlockId) -> Module {
        let module = &jit_module.module;
        let func = &module.funcs[self.func_id];
        let mut bb = func.bbs[bb_id].clone();

        let mut funcs = TiVec::new();

        let entry_func = Func {
            id: funcs.next_key(),
            args: 0,
            ret_type: LocalType::Type(Type::Val(ValType::FuncRef)), // TODO: Nilでも返したほうがよさそう
            locals: ti_vec![
                LocalType::Type(Type::Val(ValType::FuncRef)),
                LocalType::Type(Type::Boxed)
            ],
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![BasicBlock {
                id: BasicBlockId::from(0),
                exprs: {
                    let mut exprs = Vec::new();
                    exprs.extend([
                        ExprAssign {
                            local: None,
                            expr: Expr::InitModule,
                        },
                        ExprAssign {
                            local: Some(LocalId::from(0)),
                            expr: Expr::FuncRef(FuncId::from(1)),
                        },
                        ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(0)),
                        },
                        ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(self.jit_bbs[bb.id].global, LocalId::from(1)),
                        },
                    ]);
                    exprs
                },
                next: BasicBlockNext::Return(LocalId::from(0)),
            },],
            jit_strategy: FuncJitStrategy::Never,
        };
        funcs.push(entry_func);

        let mut new_locals = TiVec::new();
        let bb_info = &self.jit_bbs[bb_id].info;

        new_locals.extend(bb_info.arg_types(func));
        new_locals.extend(bb_info.define_types(func));

        for (local_id, _) in bb.local_usages_mut() {
            *local_id = bb_info.from_original_locals_mapping[local_id];
        }

        let local_offset = new_locals.len();

        new_locals.push(LocalType::Type(Type::Boxed)); // boxed bb1_ref
        new_locals.push(LocalType::Type(Type::Val(ValType::FuncRef))); // bb1_ref

        let boxed_func_ref = LocalId::from(local_offset);
        let func_ref: LocalId = LocalId::from(local_offset + 1);
        let mut extra_bbs = Vec::new();
        let new_bb = {
            let mut exprs = Vec::new();
            for expr in bb.exprs.iter() {
                // FuncRefとCall命令はget global命令に置き換えられる
                match *expr {
                    ExprAssign {
                        local,
                        expr: Expr::FuncRef(id),
                    } => {
                        exprs.push(ExprAssign {
                            local: Some(boxed_func_ref),
                            expr: Expr::GlobalGet(jit_module.func_to_globals[id]),
                        });
                        exprs.push(ExprAssign {
                            local: local,
                            expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                        });
                    }
                    ExprAssign {
                        local,
                        expr: Expr::Call(ExprCall { func_id, ref args }),
                    } => {
                        exprs.push(ExprAssign {
                            local: Some(boxed_func_ref),
                            expr: Expr::GlobalGet(jit_module.func_to_globals[func_id]),
                        });
                        exprs.push(ExprAssign {
                            local: Some(func_ref),
                            expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                        });
                        exprs.push(ExprAssign {
                            local: local,
                            expr: Expr::CallRef(ExprCallRef {
                                func: func_ref,
                                args: args.clone(),
                                func_type: module.funcs[func_id].func_type(),
                            }),
                        });
                    }
                    ref expr => {
                        exprs.push(expr.clone());
                    }
                }
            }

            // nextがtail callならexpr::callと同じようにget globalに置き換える
            // nextがif/jumpなら、BBに対応する関数へのジャンプに置き換える
            let next = match bb.next {
                BasicBlockNext::TailCall(ExprCall { func_id, ref args }) => {
                    exprs.push(ExprAssign {
                        local: Some(boxed_func_ref),
                        expr: Expr::GlobalGet(jit_module.func_to_globals[func_id]),
                    });
                    exprs.push(ExprAssign {
                        local: Some(func_ref),
                        expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                    });
                    BasicBlockNext::TailCallRef(ExprCallRef {
                        func: func_ref,
                        args: args.clone(),
                        func_type: module.funcs[func_id].func_type(),
                    })
                }
                BasicBlockNext::If(cond, then_bb, else_bb) => {
                    let then_locals_to_pass = calculate_args_to_pass(
                        &self.jit_bbs[bb_id].info,
                        &self.jit_bbs[then_bb].info,
                    );
                    let else_locals_to_pass = calculate_args_to_pass(
                        &self.jit_bbs[bb_id].info,
                        &self.jit_bbs[else_bb].info,
                    );

                    let then_bb_new = BasicBlock {
                        id: BasicBlockId::from(1),
                        exprs: vec![
                            ExprAssign {
                                local: Some(boxed_func_ref),
                                expr: Expr::GlobalGet(self.jit_bbs[then_bb].global),
                            },
                            ExprAssign {
                                local: Some(func_ref),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                            },
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: func_ref,
                            args: then_locals_to_pass,
                            func_type: FuncType {
                                args: self.jit_bbs[then_bb].info.arg_types(func),
                                ret: func.ret_type,
                            },
                        }),
                    };

                    let else_bb_new = BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![
                            ExprAssign {
                                local: Some(boxed_func_ref),
                                expr: Expr::GlobalGet(self.jit_bbs[else_bb].global),
                            },
                            ExprAssign {
                                local: Some(func_ref),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                            },
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: func_ref,
                            args: else_locals_to_pass,
                            func_type: FuncType {
                                args: self.jit_bbs[else_bb].info.arg_types(func),
                                ret: func.ret_type,
                            },
                        }),
                    };

                    extra_bbs.push(then_bb_new);
                    extra_bbs.push(else_bb_new);

                    BasicBlockNext::If(cond, BasicBlockId::from(1), BasicBlockId::from(2))
                }
                BasicBlockNext::Jump(target_bb) => {
                    let args_to_pass = calculate_args_to_pass(
                        &self.jit_bbs[bb_id].info,
                        &self.jit_bbs[target_bb].info,
                    );

                    exprs.push(ExprAssign {
                        local: Some(boxed_func_ref),
                        expr: Expr::GlobalGet(self.jit_bbs[target_bb].global),
                    });
                    exprs.push(ExprAssign {
                        local: Some(func_ref),
                        expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                    });

                    BasicBlockNext::TailCallRef(ExprCallRef {
                        func: func_ref,
                        args: args_to_pass,
                        func_type: FuncType {
                            args: self.jit_bbs[target_bb].info.arg_types(func),
                            ret: func.ret_type,
                        },
                    })
                }
                next @ (BasicBlockNext::TailCallRef(_) | BasicBlockNext::Return(_)) => next.clone(),
            };

            BasicBlock {
                id: BasicBlockId::from(0),
                exprs,
                next,
            }
        };
        let new_bb = bb_optimizer::remove_box(&mut new_locals, new_bb, &ti_vec![], &ti_vec![]);
        let mut body_func = Func {
            id: funcs.next_key(),
            args: self.jit_bbs[bb_id].info.args.len(),
            ret_type: func.ret_type,
            locals: new_locals,
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![new_bb],
            jit_strategy: FuncJitStrategy::Never,
        };

        body_func.bbs.extend(extra_bbs);

        funcs.push(body_func);

        Module {
            globals: self.globals.clone(),
            funcs,
            entry: FuncId::from(0),
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }
}

#[derive(Debug)]
struct JitBB {
    bb_id: BasicBlockId,
    global: GlobalId,
    info: BBInfo,
}

#[derive(Debug, Clone, Default)]
struct AnalyzeResult {
    defined_locals: FxHashSet<LocalId>,
    used_locals: FxHashSet<LocalId>,
}

fn analyze_locals(func: &Func) -> TiVec<BasicBlockId, AnalyzeResult> {
    let mut results = TiVec::new();

    for bb in func.bbs.iter() {
        let mut defined = FxHashSet::default();
        let mut used = FxHashSet::default();

        for (local_id, flag) in bb.local_usages() {
            match flag {
                LocalFlag::Defined => {
                    defined.insert(*local_id);
                }
                LocalFlag::Used => {
                    used.insert(*local_id);
                }
            }
        }

        results.push(AnalyzeResult {
            defined_locals: defined,
            used_locals: used,
        });
    }

    // BBは前方ジャンプがないことを仮定している
    // 複雑な制御フローを持つ場合はトポロジカルソートなどが必要
    let bb_ids = func.bbs.iter().map(|bb| bb.id).collect::<Vec<_>>();

    // defineの集計は前から行う
    // 自分より前のブロックで定義済みの関数
    let mut prev_defines = TiVec::new();
    for _ in bb_ids.iter() {
        prev_defines.push(FxHashSet::<LocalId>::default());
    }
    for &bb_id in bb_ids.iter() {
        let result = &mut results[bb_id];
        for prev_define in prev_defines[bb_id].iter() {
            let removed = result.defined_locals.remove(prev_define);
            if removed {
                result.used_locals.insert(*prev_define);
            }
        }

        let prev_define = prev_defines[bb_id].clone();
        for succ in func.bbs[bb_id].next.successors() {
            prev_defines[succ].extend(&result.defined_locals);
            prev_defines[succ].extend(&prev_define);
        }
    }

    for &bb_id in bb_ids.iter().rev() {
        let mut result = results[bb_id].clone();

        for succ in func.bbs[bb_id].next.successors() {
            let succ_result = &results[succ];
            result.used_locals.extend(&succ_result.used_locals);
        }

        for define in &result.defined_locals {
            result.used_locals.remove(define);
        }

        results[bb_id] = result;
    }

    // エントリーポイントの例外的処理
    results[func.bb_entry].used_locals =
        (0..func.args).map(LocalId::from).collect::<FxHashSet<_>>();
    for i in 0..func.args {
        results[func.bb_entry]
            .defined_locals
            .remove(&LocalId::from(i));
    }

    results
}

#[derive(Debug, Clone, Default)]
struct BBInfo {
    args: Vec<LocalId>,
    defines: Vec<LocalId>,
    type_params: TiVec<TypeParamId, LocalId>,
    to_original_locals_mapping: TiVec<LocalId, LocalId>,
    from_original_locals_mapping: FxHashMap<LocalId, LocalId>,
}

impl BBInfo {
    fn arg_types(&self, func: &Func) -> Vec<LocalType> {
        self.args
            .iter()
            .map(|&arg| func.locals[self.to_original_locals_mapping[arg]])
            .collect()
    }

    fn define_types(&self, func: &Func) -> Vec<LocalType> {
        self.defines
            .iter()
            .map(|&define| func.locals[self.to_original_locals_mapping[define]])
            .collect()
    }
}

fn calculate_bb_info(
    locals: &TiVec<LocalId, LocalType>,
    analyze_results: TiVec<BasicBlockId, AnalyzeResult>,
) -> TiVec<BasicBlockId, BBInfo> {
    let mut bb_info = TiVec::new();

    for result in analyze_results.into_iter() {
        let mut to_original_locals_mapping = TiVec::new();

        let mut original_id_args = result.used_locals.into_iter().collect::<Vec<_>>();
        original_id_args.sort();
        let mut original_id_defines = result.defined_locals.into_iter().collect::<Vec<_>>();
        original_id_defines.sort();

        let mut args = Vec::new();
        let mut defines = Vec::new();

        for original_id_arg in original_id_args {
            let local_id = to_original_locals_mapping.push_and_get_key(original_id_arg);
            args.push(local_id);
        }

        for original_id_define in original_id_defines {
            let local_id = to_original_locals_mapping.push_and_get_key(original_id_define);
            defines.push(local_id);
        }

        let mut type_params = TiVec::new();
        for &arg in &args {
            if let LocalType::Type(Type::Boxed) = locals[arg] {
                type_params.push(arg);
            }
        }

        let from_original_locals_mapping = to_original_locals_mapping
            .iter_enumerated()
            .map(|(k, &v)| (v, k))
            .collect::<FxHashMap<_, _>>();

        let info = BBInfo {
            args,
            defines,
            type_params,
            to_original_locals_mapping,
            from_original_locals_mapping,
        };

        bb_info.push(info);
    }

    bb_info
}

fn calculate_args_to_pass(caller: &BBInfo, callee: &BBInfo) -> Vec<LocalId> {
    let mut args_to_pass = Vec::new();
    for &arg in &callee.args {
        args_to_pass
            .push(caller.from_original_locals_mapping[&callee.to_original_locals_mapping[arg]]);
    }
    args_to_pass
}
