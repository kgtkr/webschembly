use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::{TiVec, ti_vec};

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
    bb_to_globals: TiVec<BasicBlockId, GlobalId>,
    bb_infos: TiVec<BasicBlockId, BBInfo>,
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

        let bb_infos = calculate_bb_info(analyze_locals(func));

        let globals = {
            let mut globals = jit_module.globals.clone();
            globals.extend(bb_to_globals.iter());
            globals
        };

        Self {
            func_id,
            bb_to_globals,
            bb_infos,
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
                    for (bb_id, &bb_global) in self.bb_to_globals.iter_enumerated() {
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(0)),
                            expr: Expr::FuncRef(FuncId::from(2 + usize::from(bb_id))),
                        });
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(0)),
                        });
                        exprs.push(ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(bb_global, LocalId::from(1)),
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
                        expr: Expr::GlobalGet(self.bb_to_globals[func.bb_entry]),
                    },
                    ExprAssign {
                        local: Some(LocalId::from(func.args + 1)),
                        expr: Expr::Unbox(ValType::FuncRef, LocalId::from(func.args),),
                    },
                ],
                next: BasicBlockNext::TailCallRef(ExprCallRef {
                    func: LocalId::from(func.args + 1),
                    args: self.bb_infos[func.bb_entry].args.clone(),
                    func_type: FuncType {
                        args: self.bb_infos[func.bb_entry]
                            .args
                            .iter()
                            .map(|&arg| func.locals[arg])
                            .collect::<Vec<_>>(),
                        ret: func.ret_type,
                    },
                })
            },],
            jit_strategy: FuncJitStrategy::BasicBlock,
        };

        funcs.push(body_func);

        for (bb_id, &bb_global) in self.bb_to_globals.iter_enumerated() {
            /*
            func bb0_stub(...) {
                if bb0_ref == bb0_stub
                    instantiate_bb(...)
                bb0 <- get_global bb0_ref
                bb0(...)
            }
            */
            let bb_info = &self.bb_infos[bb_id];
            let mut locals = TiVec::new();
            locals.extend(bb_info.args.iter().map(|&arg| func.locals[arg]));
            locals.extend([
                func.ret_type(),
                LocalType::Type(Type::Boxed), // boxed bb0_ref
                LocalType::Type(Type::Val(ValType::FuncRef)), // bb0_ref
                LocalType::Type(Type::Boxed), // bb0_stub
                LocalType::Type(Type::Val(ValType::Bool)), // bb0_ref != bb0_stub
            ]);

            let func = Func {
                id: funcs.next_key(),
                args: bb_info.args.len(),
                ret_type: func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: ti_vec![
                    BasicBlock {
                        id: BasicBlockId::from(0),
                        exprs: vec![
                            ExprAssign {
                                local: Some(LocalId::from(bb_info.args.len() + 1)),
                                expr: Expr::GlobalGet(bb_global),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(bb_info.args.len() + 3)),
                                expr: Expr::FuncRef(FuncId::from(2 + usize::from(bb_id))),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(bb_info.args.len() + 4)),
                                expr: Expr::Eq(
                                    LocalId::from(bb_info.args.len() + 1),
                                    LocalId::from(bb_info.args.len() + 3),
                                ),
                            },
                        ],
                        next: BasicBlockNext::If(
                            LocalId::from(bb_info.args.len() + 4),
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
                                local: Some(LocalId::from(bb_info.args.len() + 1)),
                                expr: Expr::GlobalGet(bb_global),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(bb_info.args.len() + 2)),
                                expr: Expr::Unbox(
                                    ValType::FuncRef,
                                    LocalId::from(bb_info.args.len() + 1),
                                ),
                            }
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: LocalId::from(bb_info.args.len() + 2),
                            args: (0..bb_info.args.len())
                                .map(LocalId::from)
                                .collect::<Vec<_>>(),
                            func_type: FuncType {
                                args: bb_info
                                    .args
                                    .iter()
                                    .map(|&arg| func.locals[arg])
                                    .collect::<Vec<_>>(),
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
        let bb_info = &self.bb_infos[bb_id];

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
                            expr: Expr::GlobalSet(self.bb_to_globals[bb.id], LocalId::from(1)),
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

        for &arg in &bb_info.args {
            new_locals.push(func.locals[arg]);
        }

        for &define in &bb_info.defines {
            new_locals.push(func.locals[define]);
        }

        for (local_id, _) in bb.local_usages_mut() {
            *local_id = bb_info.locals_mapping[local_id];
        }

        let local_offset = new_locals.len();

        new_locals.push(LocalType::Type(Type::Boxed)); // boxed bb1_ref
        new_locals.push(LocalType::Type(Type::Val(ValType::FuncRef))); // bb1_ref

        let boxed_func_ref = LocalId::from(local_offset);
        let func_ref: LocalId = LocalId::from(local_offset + 1);
        let mut extra_bbs = Vec::new();
        let mut body_func = Func {
            id: funcs.next_key(),
            args: bb_info.args.len(),
            ret_type: func.ret_type,
            locals: new_locals,
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![{
                let mut exprs = Vec::new();
                for expr in bb.exprs.iter() {
                    // FuncRefとCall命令はget global命令に置き換えられる
                    match expr.expr {
                        Expr::FuncRef(id) => {
                            exprs.push(ExprAssign {
                                local: Some(boxed_func_ref),
                                expr: Expr::GlobalGet(jit_module.func_to_globals[id]),
                            });
                            exprs.push(ExprAssign {
                                local: expr.local,
                                expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                            });
                        }
                        Expr::Call(ExprCall { func_id, ref args }) => {
                            exprs.push(ExprAssign {
                                local: Some(boxed_func_ref),
                                expr: Expr::GlobalGet(jit_module.func_to_globals[func_id]),
                            });
                            exprs.push(ExprAssign {
                                local: Some(func_ref),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                            });
                            exprs.push(ExprAssign {
                                local: expr.local,
                                expr: Expr::CallRef(ExprCallRef {
                                    func: func_ref,
                                    args: args.clone(),
                                    func_type: module.funcs[func_id].func_type(),
                                }),
                            });
                        }
                        _ => {
                            exprs.push(expr.clone());
                        }
                    }
                }

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
                        let then_locals_to_pass =
                            calculate_args_to_pass(bb_info, &self.bb_infos[then_bb]);
                        let else_locals_to_pass =
                            calculate_args_to_pass(bb_info, &self.bb_infos[else_bb]);

                        let then_bb_new = BasicBlock {
                            id: BasicBlockId::from(1),
                            exprs: vec![
                                ExprAssign {
                                    local: Some(boxed_func_ref),
                                    expr: Expr::GlobalGet(self.bb_to_globals[then_bb]),
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
                                    args: self.bb_infos[then_bb]
                                        .args
                                        .iter()
                                        .map(|&arg| func.locals[arg])
                                        .collect::<Vec<_>>(),
                                    ret: func.ret_type,
                                },
                            }),
                        };

                        let else_bb_new = BasicBlock {
                            id: BasicBlockId::from(2),
                            exprs: vec![
                                ExprAssign {
                                    local: Some(boxed_func_ref),
                                    expr: Expr::GlobalGet(self.bb_to_globals[else_bb]),
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
                                    args: self.bb_infos[else_bb]
                                        .args
                                        .iter()
                                        .map(|&arg| func.locals[arg])
                                        .collect::<Vec<_>>(),
                                    ret: func.ret_type,
                                },
                            }),
                        };

                        extra_bbs.push(then_bb_new);
                        extra_bbs.push(else_bb_new);

                        BasicBlockNext::If(cond, BasicBlockId::from(1), BasicBlockId::from(2))
                    }
                    BasicBlockNext::Jump(target_bb) => {
                        let args_to_pass =
                            calculate_args_to_pass(bb_info, &self.bb_infos[target_bb]);

                        exprs.push(ExprAssign {
                            local: Some(boxed_func_ref),
                            expr: Expr::GlobalGet(self.bb_to_globals[target_bb]),
                        });
                        exprs.push(ExprAssign {
                            local: Some(func_ref),
                            expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                        });

                        BasicBlockNext::TailCallRef(ExprCallRef {
                            func: func_ref,
                            args: args_to_pass,
                            func_type: FuncType {
                                args: self.bb_infos[target_bb]
                                    .args
                                    .iter()
                                    .map(|&arg| func.locals[arg])
                                    .collect::<Vec<_>>(),
                                ret: func.ret_type,
                            },
                        })
                    }
                    next @ (BasicBlockNext::TailCallRef(_) | BasicBlockNext::Return(_)) => {
                        next.clone()
                    }
                };

                BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs,
                    next,
                }
            }],
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

        if bb.id == func.bb_entry {
            for i in 0..func.args {
                defined.insert(LocalId::from(i));
            }
        }

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
    // argsとdefinesはorig func_id
    args: Vec<LocalId>,
    defines: Vec<LocalId>,
    // orig func_id -> new func_id
    locals_mapping: FxHashMap<LocalId, LocalId>,
}

fn calculate_bb_info(
    analyze_results: TiVec<BasicBlockId, AnalyzeResult>,
) -> TiVec<BasicBlockId, BBInfo> {
    let mut bb_info = TiVec::new();

    for result in analyze_results.into_iter() {
        let mut args = result.used_locals.into_iter().collect::<Vec<_>>();
        args.sort();
        let mut defines = result.defined_locals.into_iter().collect::<Vec<_>>();
        defines.sort();
        let mut info = BBInfo {
            args,
            defines,
            locals_mapping: FxHashMap::default(),
        };

        let mut local_id = LocalId::from(0);
        for &arg in &info.args {
            info.locals_mapping.insert(arg, local_id);
            local_id = LocalId::from(usize::from(local_id) + 1);
        }
        for &define in &info.defines {
            info.locals_mapping.insert(define, local_id);
            local_id = LocalId::from(usize::from(local_id) + 1);
        }

        bb_info.push(info);
    }

    bb_info
}

fn calculate_args_to_pass(caller: &BBInfo, callee: &BBInfo) -> Vec<LocalId> {
    let mut args_to_pass = Vec::new();
    for &arg in &callee.args {
        args_to_pass.push(caller.locals_mapping[&arg]);
    }
    args_to_pass
}
