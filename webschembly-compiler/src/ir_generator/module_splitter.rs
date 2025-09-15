use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::*;
use crate::ir_generator::IrGenerator;
use typed_index_collections::{TiVec, ti_vec};

pub fn split_and_register_module(ir_generator: &mut IrGenerator, module: Module) -> ModuleId {
    let mut splitter = ModuleSplitter::new(ir_generator, &module);
    splitter.split_and_register_module(module);
    splitter.entry_module_id
}

#[derive(Debug)]
struct ModuleSplitter<'a> {
    ir_generator: &'a mut IrGenerator,
    func_ref_globals: TiVec<FuncId, GlobalId>,
    globals: FxHashSet<GlobalId>,
    func_types: TiVec<FuncId, FuncType>,
    entry_module_id: ModuleId,
    module_ids: TiVec<FuncId, ModuleId>,
}

impl<'a> ModuleSplitter<'a> {
    fn new(ir_generator: &'a mut IrGenerator, module: &Module) -> Self {
        let func_ref_globals = module
            .funcs
            .iter()
            .map(|_| ir_generator.gen_global_id())
            .collect::<TiVec<FuncId, _>>();

        let globals = {
            let mut globals = module.globals.clone();
            globals.extend(func_ref_globals.iter());
            globals
        };

        let func_types = module
            .funcs
            .iter()
            .map(|func| func.func_type())
            .collect::<TiVec<FuncId, _>>();

        let entry_module_id = ir_generator.alloc_module_id();
        let module_ids = module
            .funcs
            .iter()
            .map(|_| ir_generator.alloc_module_id())
            .collect::<TiVec<FuncId, _>>();
        Self {
            ir_generator,
            func_ref_globals,
            globals,
            func_types,
            entry_module_id,
            module_ids,
        }
    }

    fn split_and_register_module(&mut self, module: Module) {
        let entry_module = self.generate_entry_module(&module);
        self.ir_generator
            .set_module(self.entry_module_id, entry_module);

        // 各関数のモジュール
        for func in module.funcs {
            let module_id = self.module_ids[func.id];
            let module = self.generate_func_module(func);

            self.ir_generator.set_module(module_id, module);
        }
    }

    fn generate_entry_module(&self, module: &Module) -> Module {
        // entry関数もあるので+1してる
        let stub_func_ids = module
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
                    for func_id in &module.funcs {
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::FuncRef(stub_func_ids[func_id.id]),
                        });
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(2)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(1)),
                        });
                        exprs.push(ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(
                                self.func_ref_globals[func_id.id],
                                LocalId::from(2),
                            ),
                        });
                    }
                    exprs
                },
                next: BasicBlockNext::TailCall(ExprCall {
                    func_id: stub_func_ids[module.entry],
                    args: vec![],
                })
            },],
            jit_strategy: FuncJitStrategy::Never,
        };
        funcs.push(func);
        for func in module.funcs.iter() {
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
                                expr: Expr::GlobalGet(self.func_ref_globals[func.id]),
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
                            expr: Expr::InstantiateModule(self.module_ids[func.id]),
                        }],
                        next: BasicBlockNext::Jump(BasicBlockId::from(2)),
                    },
                    BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![
                            ExprAssign {
                                local: Some(LocalId::from(func.args + 1)),
                                expr: Expr::GlobalGet(self.func_ref_globals[func.id]),
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

    fn generate_func_module(&self, func: Func) -> Module {
        /*
        以下に対応するモジュールを生成
        func entry() {
            set_global f0_ref f0
        }

        func f0() {
            f1 <- get_global f1_ref
            f1()
        }

        */

        let mut funcs = TiVec::<FuncId, _>::new();
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
                exprs: vec![
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
                        expr: Expr::GlobalSet(self.func_ref_globals[func.id], LocalId::from(1)),
                    }
                ],
                next: BasicBlockNext::Return(LocalId::from(0)),
            },],
            jit_strategy: FuncJitStrategy::Never,
        };
        funcs.push(entry_func);
        let boxed_func_ref = func.locals.next_key();
        let func_ref = LocalId::from(usize::from(boxed_func_ref) + 1);
        let body_func = Func {
            id: funcs.next_key(),
            args: func.args,
            ret_type: func.ret_type,
            locals: {
                let mut locals = func.locals;
                locals.push(LocalType::Type(Type::Boxed));
                locals.push(LocalType::Type(Type::Val(ValType::FuncRef)));
                locals
            },
            bb_entry: func.bb_entry,
            bbs: func
                .bbs
                .into_iter()
                .map(|bb| {
                    let mut exprs = Vec::new();
                    for expr in bb.exprs {
                        // FuncRefとCall命令はget global命令に置き換えられる
                        match expr.expr {
                            Expr::FuncRef(id) => {
                                exprs.push(ExprAssign {
                                    local: Some(boxed_func_ref),
                                    expr: Expr::GlobalGet(self.func_ref_globals[id]),
                                });
                                exprs.push(ExprAssign {
                                    local: expr.local,
                                    expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                                });
                            }
                            Expr::Call(ExprCall { func_id, args }) => {
                                exprs.push(ExprAssign {
                                    local: Some(boxed_func_ref),
                                    expr: Expr::GlobalGet(self.func_ref_globals[func_id]),
                                });
                                exprs.push(ExprAssign {
                                    local: Some(func_ref),
                                    expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                                });
                                exprs.push(ExprAssign {
                                    local: expr.local,
                                    expr: Expr::CallRef(ExprCallRef {
                                        func: func_ref,
                                        args,
                                        func_type: self.func_types[func_id].clone(),
                                    }),
                                });
                            }
                            _ => {
                                exprs.push(expr);
                            }
                        }
                    }

                    let next = match bb.next {
                        BasicBlockNext::TailCall(ExprCall { func_id, args }) => {
                            exprs.push(ExprAssign {
                                local: Some(boxed_func_ref),
                                expr: Expr::GlobalGet(self.func_ref_globals[func_id]),
                            });
                            exprs.push(ExprAssign {
                                local: Some(func_ref),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                            });
                            BasicBlockNext::TailCallRef(ExprCallRef {
                                func: func_ref,
                                args,
                                func_type: self.func_types[func_id].clone(),
                            })
                        }
                        next @ (BasicBlockNext::TailCallRef(_)
                        | BasicBlockNext::Return(_)
                        | BasicBlockNext::If(_, _, _)
                        | BasicBlockNext::Jump(_)) => next,
                    };

                    BasicBlock {
                        id: bb.id,
                        exprs,
                        next,
                    }
                })
                .collect(),
            jit_strategy: FuncJitStrategy::Never,
        };

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
