use std::borrow::Cow;

use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::{TiVec, ti_vec};

use super::bb_optimizer;
use crate::fxbihashmap::FxBiHashMap;
use crate::ir::*;
use crate::ir_generator::GlobalManager;
use crate::ir_generator::bb_optimizer::NextTypeArg;

#[derive(Debug)]
pub struct Jit {
    jit_module: TiVec<ModuleId, JitModule>,
    global_layout: GlobalLayout,
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
            global_layout: GlobalLayout::new(),
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
            .generate_stub_module(&self.global_layout, &self.jit_module[module_id])
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: ModuleId,
        func_id: FuncId,
        bb_id: BasicBlockId,
        index: usize,
    ) -> Module {
        let jit_module = &self.jit_module[module_id];
        let jit_func = self.jit_module[module_id].jit_funcs[func_id]
            .as_ref()
            .unwrap();
        jit_func.generate_bb_module(jit_module, bb_id, index, &mut self.global_layout) // TODO: index
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
            args: vec![],
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
                args: func.args.clone(),
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
                                local: Some(LocalId::from(func.args.len() + 1)),
                                expr: Expr::GlobalGet(self.func_to_globals[func.id]),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(func.args.len() + 3)),
                                expr: Expr::FuncRef(stub_func_ids[func.id]),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(func.args.len() + 4)),
                                expr: Expr::Eq(
                                    LocalId::from(func.args.len() + 1),
                                    LocalId::from(func.args.len() + 3),
                                ),
                            },
                        ],
                        next: BasicBlockNext::If(
                            LocalId::from(func.args.len() + 4),
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
                                local: Some(LocalId::from(func.args.len() + 1)),
                                expr: Expr::GlobalGet(self.func_to_globals[func.id]),
                            },
                            ExprAssign {
                                local: Some(LocalId::from(func.args.len() + 2)),
                                expr: Expr::Unbox(
                                    ValType::FuncRef,
                                    LocalId::from(func.args.len() + 1)
                                ),
                            },
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: LocalId::from(func.args.len() + 2),
                            args: func.args.clone(),
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

    fn generate_stub_module(&self, global_layout: &GlobalLayout, jit_module: &JitModule) -> Module {
        let module = &jit_module.module;
        let func = &module.funcs[self.func_id];

        let mut funcs = TiVec::<FuncId, _>::new();
        /*
        func entry() {
            set_global f0_ref f0
            set_global bb0_ref [bb0_stub, bb0_stub, ..., bb0_stub]
            set_global bb1_ref [bb1_stub, bb1_stub, ..., bb1_stub]
        }
        */
        let entry_func = {
            let mut locals = TiVec::new();
            let func_ref_local =
                locals.push_and_get_key(LocalType::Type(Type::Val(ValType::FuncRef)));
            let vector_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Vector)));
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));

            Func {
                id: funcs.next_key(),
                args: vec![],
                ret_type: LocalType::Type(Type::Val(ValType::FuncRef)), // TODO: Nilでも返したほうがよさそう
                locals,
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
                                local: Some(func_ref_local),
                                expr: Expr::FuncRef(FuncId::from(1)),
                            },
                            ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::Box(ValType::FuncRef, func_ref_local),
                            },
                            ExprAssign {
                                local: None,
                                expr: Expr::GlobalSet(
                                    jit_module.func_to_globals[func.id],
                                    boxed_local,
                                ),
                            },
                        ]);
                        for jit_bb in self.jit_bbs.iter() {
                            exprs.push(ExprAssign {
                                local: Some(func_ref_local),
                                expr: Expr::FuncRef(FuncId::from(2 + usize::from(jit_bb.bb_id))),
                            });
                            exprs.push(ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::Box(ValType::FuncRef, func_ref_local),
                            });
                            // TODO: Boxedの列であるVectorに入れるのは非効率なのでFuncTableのようなものが欲しい
                            exprs.push(ExprAssign {
                                local: Some(vector_local),
                                expr: Expr::Vector(vec![boxed_local; GLOBAL_LAYOUT_MAX_SIZE]),
                            });
                            exprs.push(ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::Box(ValType::Vector, vector_local),
                            });
                            exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::GlobalSet(jit_bb.global, boxed_local),
                            });
                        }
                        exprs
                    },
                    next: BasicBlockNext::Return(func_ref_local),
                },],
                jit_strategy: FuncJitStrategy::Never,
            }
        };
        funcs.push(entry_func);

        /*
        func f0(...) {
            bb0 <- get_global bb0_ref
            bb0[index](...)
        }
        */
        let entry_bb_info = &self.jit_bbs[func.bb_entry].info;
        let body_func = {
            let mut locals = TiVec::new();
            locals.extend(func.arg_types());
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));
            let func_ref_local =
                locals.push_and_get_key(LocalType::Type(Type::Val(ValType::FuncRef)));
            let vector_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Vector)));
            let index_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Int)));

            Func {
                id: funcs.next_key(),
                args: func.args.clone(),
                ret_type: func.ret_type,
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: ti_vec![BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs: vec![
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::GlobalGet(self.jit_bbs[func.bb_entry].global),
                        },
                        ExprAssign {
                            local: Some(vector_local),
                            expr: Expr::Unbox(ValType::Vector, boxed_local,),
                        },
                        ExprAssign {
                            local: Some(index_local),
                            expr: Expr::Int(GLOBAL_LAYOUT_DEFAULT_INDEX as i64),
                        },
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::VectorRef(vector_local, index_local),
                        },
                        ExprAssign {
                            local: Some(func_ref_local),
                            expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                        },
                    ],
                    next: BasicBlockNext::TailCallRef(ExprCallRef {
                        func: func_ref_local,
                        args: entry_bb_info
                            .args
                            .iter()
                            .map(|&arg| entry_bb_info.to_original_locals_mapping[arg])
                            .collect(),
                        func_type: FuncType {
                            args: entry_bb_info
                                .arg_types(func, &ti_vec![None; entry_bb_info.type_params.len()]),
                            ret: func.ret_type,
                        },
                    })
                },],
                jit_strategy: FuncJitStrategy::BasicBlock,
            }
        };

        funcs.push(body_func);

        for jit_bb in self.jit_bbs.iter() {
            let func = Self::generate_bb_stub_func(
                global_layout,
                jit_module,
                jit_bb,
                func,
                funcs.next_key(),
                GLOBAL_LAYOUT_DEFAULT_INDEX,
            );
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

    fn generate_bb_stub_func(
        global_layout: &GlobalLayout,
        jit_module: &JitModule,
        jit_bb: &JitBB,
        func: &Func,
        id: FuncId,
        index: usize,
    ) -> Func {
        /*
        func bb0_stub(...) {
            bb0 <- get_global bb0_ref
            if bb0[index] == bb0_stub
                instantiate_bb(..., index)
            bb0 <- get_global bb0_ref
            bb0(...)
        }
        */
        let type_args = &*global_layout.from_idx(index, jit_bb.info.type_params.len());
        let mut locals = TiVec::new();
        locals.extend(jit_bb.info.arg_types(func, type_args));
        let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed)); // boxed bb0_ref
        let func_ref_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::FuncRef))); // bb0_ref
        let stub_local = locals.push_and_get_key(LocalType::Type(Type::Boxed)); // bb0_stub
        let bool_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Bool))); // bb0_ref != bb0_stub
        let vector_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Vector)));
        let index_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Int)));

        Func {
            id,
            args: jit_bb.info.args.clone(),
            ret_type: func.ret_type,
            locals,
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![
                BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs: vec![
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::GlobalGet(jit_bb.global),
                        },
                        ExprAssign {
                            local: Some(vector_local),
                            expr: Expr::Unbox(ValType::Vector, boxed_local,),
                        },
                        ExprAssign {
                            local: Some(index_local),
                            expr: Expr::Int(index as i64),
                        },
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::VectorRef(vector_local, index_local),
                        },
                        ExprAssign {
                            local: Some(stub_local),
                            expr: Expr::FuncRef(id),
                        },
                        ExprAssign {
                            local: Some(bool_local),
                            expr: Expr::Eq(boxed_local, stub_local,),
                        },
                    ],
                    next: BasicBlockNext::If(
                        bool_local,
                        BasicBlockId::from(1),
                        BasicBlockId::from(2),
                    ),
                },
                BasicBlock {
                    id: BasicBlockId::from(1),
                    exprs: vec![ExprAssign {
                        local: None,
                        expr: Expr::InstantiateBB(
                            jit_module.module_id,
                            func.id,
                            jit_bb.bb_id,
                            index_local
                        ),
                    }],
                    next: BasicBlockNext::Jump(BasicBlockId::from(2)),
                },
                BasicBlock {
                    id: BasicBlockId::from(2),
                    exprs: vec![
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::GlobalGet(jit_bb.global),
                        },
                        ExprAssign {
                            local: Some(vector_local),
                            expr: Expr::Unbox(ValType::Vector, boxed_local,),
                        },
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::VectorRef(vector_local, index_local),
                        },
                        ExprAssign {
                            local: Some(func_ref_local),
                            expr: Expr::Unbox(ValType::FuncRef, boxed_local,),
                        }
                    ],
                    next: BasicBlockNext::TailCallRef(ExprCallRef {
                        func: func_ref_local,
                        args: jit_bb.info.args.clone(),
                        func_type: FuncType {
                            args: jit_bb.info.arg_types(func, type_args),
                            ret: func.ret_type,
                        },
                    }),
                },
            ],
            jit_strategy: FuncJitStrategy::Never,
        }
    }

    fn generate_bb_module(
        &self,
        jit_module: &JitModule,
        bb_id: BasicBlockId,
        index: usize,
        global_layout: &mut GlobalLayout,
    ) -> Module {
        let mut required_stubs = Vec::new();

        let type_args = &*global_layout.from_idx(index, self.jit_bbs[bb_id].info.type_params.len());
        let module = &jit_module.module;
        let func = &module.funcs[self.func_id];
        let mut bb = func.bbs[bb_id].clone();

        let mut funcs = TiVec::new();

        let mut new_locals = TiVec::new();
        let bb_info = &self.jit_bbs[bb_id].info;

        new_locals.extend(bb_info.arg_types(func, type_args));
        new_locals.extend(bb_info.define_types(func));

        for (local_id, _) in bb.local_usages_mut() {
            *local_id = bb_info.from_original_locals_mapping[local_id];
        }

        let bb = bb_optimizer::remove_move(&new_locals, bb, &bb_info.args);
        let (bb, next_type_args) = bb_optimizer::remove_box(
            &mut new_locals,
            bb,
            &bb_info.type_params,
            type_args,
            &bb_info.args,
        );

        let boxed_local = new_locals.push_and_get_key(LocalType::Type(Type::Boxed)); // boxed bb1_ref
        let func_ref_local =
            new_locals.push_and_get_key(LocalType::Type(Type::Val(ValType::FuncRef))); // bb1_ref
        let index_local = new_locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Int)));
        let vector_local = new_locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Vector)));

        let mut extra_bbs = Vec::new();

        let bb = {
            let mut exprs = Vec::new();
            for expr in bb.exprs.iter() {
                // FuncRefとCall命令はget global命令に置き換えられる
                match *expr {
                    ExprAssign {
                        local,
                        expr: Expr::FuncRef(id),
                    } => {
                        exprs.push(ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::GlobalGet(jit_module.func_to_globals[id]),
                        });
                        exprs.push(ExprAssign {
                            local,
                            expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                        });
                    }
                    ExprAssign {
                        local,
                        expr: Expr::Call(ExprCall { func_id, ref args }),
                    } => {
                        exprs.push(ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::GlobalGet(jit_module.func_to_globals[func_id]),
                        });
                        exprs.push(ExprAssign {
                            local: Some(func_ref_local),
                            expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                        });
                        exprs.push(ExprAssign {
                            local,
                            expr: Expr::CallRef(ExprCallRef {
                                func: func_ref_local,
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
                        local: Some(boxed_local),
                        expr: Expr::GlobalGet(jit_module.func_to_globals[func_id]),
                    });
                    exprs.push(ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                    });
                    BasicBlockNext::TailCallRef(ExprCallRef {
                        func: func_ref_local,
                        args: args.clone(),
                        func_type: module.funcs[func_id].func_type(),
                    })
                }
                BasicBlockNext::If(cond, then_bb, else_bb) => {
                    let (then_locals_to_pass, then_type_args) = calculate_args_to_pass(
                        &self.jit_bbs[bb_id].info,
                        &self.jit_bbs[then_bb].info,
                        &next_type_args,
                    );
                    let then_index = global_layout.to_idx(&then_type_args);
                    required_stubs.push((then_bb, then_index));
                    let (else_locals_to_pass, else_type_args) = calculate_args_to_pass(
                        &self.jit_bbs[bb_id].info,
                        &self.jit_bbs[else_bb].info,
                        &next_type_args,
                    );
                    let else_index = global_layout.to_idx(&else_type_args);
                    required_stubs.push((else_bb, else_index));

                    let then_bb_new = BasicBlock {
                        id: BasicBlockId::from(1),
                        exprs: vec![
                            ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::GlobalGet(self.jit_bbs[then_bb].global),
                            },
                            ExprAssign {
                                local: Some(vector_local),
                                expr: Expr::Unbox(ValType::Vector, boxed_local),
                            },
                            ExprAssign {
                                local: Some(index_local),
                                expr: Expr::Int(then_index as i64),
                            },
                            ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::VectorRef(vector_local, index_local),
                            },
                            ExprAssign {
                                local: Some(func_ref_local),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                            },
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: func_ref_local,
                            args: then_locals_to_pass,
                            func_type: FuncType {
                                args: self.jit_bbs[then_bb].info.arg_types(func, &then_type_args),
                                ret: func.ret_type,
                            },
                        }),
                    };

                    let else_bb_new = BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![
                            ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::GlobalGet(self.jit_bbs[else_bb].global),
                            },
                            ExprAssign {
                                local: Some(vector_local),
                                expr: Expr::Unbox(ValType::Vector, boxed_local),
                            },
                            ExprAssign {
                                local: Some(index_local),
                                expr: Expr::Int(else_index as i64),
                            },
                            ExprAssign {
                                local: Some(boxed_local),
                                expr: Expr::VectorRef(vector_local, index_local),
                            },
                            ExprAssign {
                                local: Some(func_ref_local),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                            },
                        ],
                        next: BasicBlockNext::TailCallRef(ExprCallRef {
                            func: func_ref_local,
                            args: else_locals_to_pass,
                            func_type: FuncType {
                                args: self.jit_bbs[else_bb].info.arg_types(func, &else_type_args),
                                ret: func.ret_type,
                            },
                        }),
                    };

                    extra_bbs.push(then_bb_new);
                    extra_bbs.push(else_bb_new);

                    BasicBlockNext::If(cond, BasicBlockId::from(1), BasicBlockId::from(2))
                }
                BasicBlockNext::Jump(target_bb) => {
                    let (args_to_pass, type_args) = calculate_args_to_pass(
                        &self.jit_bbs[bb_id].info,
                        &self.jit_bbs[target_bb].info,
                        &next_type_args,
                    );
                    let target_index = global_layout.to_idx(&type_args);
                    required_stubs.push((target_bb, target_index));

                    exprs.push(ExprAssign {
                        local: Some(boxed_local),
                        expr: Expr::GlobalGet(self.jit_bbs[target_bb].global),
                    });
                    exprs.push(ExprAssign {
                        local: Some(vector_local),
                        expr: Expr::Unbox(ValType::Vector, boxed_local),
                    });
                    exprs.push(ExprAssign {
                        local: Some(index_local),
                        expr: Expr::Int(target_index as i64),
                    });
                    exprs.push(ExprAssign {
                        local: Some(boxed_local),
                        expr: Expr::VectorRef(vector_local, index_local),
                    });

                    exprs.push(ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::Unbox(ValType::FuncRef, boxed_local),
                    });

                    BasicBlockNext::TailCallRef(ExprCallRef {
                        func: func_ref_local,
                        args: args_to_pass,
                        func_type: FuncType {
                            args: self.jit_bbs[target_bb].info.arg_types(func, &type_args),
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

        let mut body_func = Func {
            id: funcs.next_key(),
            args: bb_info.args.clone(),
            ret_type: func.ret_type,
            locals: new_locals,
            bb_entry: BasicBlockId::from(0),
            bbs: ti_vec![bb],
            jit_strategy: FuncJitStrategy::Never,
        };

        body_func.bbs.extend(extra_bbs);
        let body_func_id = body_func.id;
        funcs.push(body_func);

        let required_stubs = required_stubs
            .iter()
            .filter(|(_, index)| *index != GLOBAL_LAYOUT_DEFAULT_INDEX)
            .map(|&(bb_id, index)| {
                let bb_stub_func_id = funcs.next_key();
                let func = Self::generate_bb_stub_func(
                    global_layout,
                    jit_module,
                    &self.jit_bbs[bb_id],
                    func,
                    bb_stub_func_id,
                    index,
                );
                funcs.push(func);
                (bb_id, index, bb_stub_func_id)
            })
            .collect::<Vec<_>>();

        let entry_func = {
            let mut locals = TiVec::new();
            let func_ref_local =
                locals.push_and_get_key(LocalType::Type(Type::Val(ValType::FuncRef)));
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));
            let vector_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Vector)));
            let index_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Int)));
            let bool_local = locals.push_and_get_key(LocalType::Type(Type::Val(ValType::Bool)));
            let boxed_stub0_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));

            let mut bbs = TiVec::new();

            bbs.push(BasicBlock {
                id: BasicBlockId::from(0),
                exprs: vec![
                    ExprAssign {
                        local: None,
                        expr: Expr::InitModule,
                    },
                    ExprAssign {
                        local: Some(index_local),
                        expr: Expr::Int(index as i64),
                    },
                    ExprAssign {
                        local: Some(boxed_local),
                        expr: Expr::GlobalGet(self.jit_bbs[bb_id].global),
                    },
                    ExprAssign {
                        local: Some(vector_local),
                        expr: Expr::Unbox(ValType::Vector, boxed_local),
                    },
                    ExprAssign {
                        local: Some(func_ref_local),
                        expr: Expr::FuncRef(body_func_id),
                    },
                    ExprAssign {
                        local: Some(boxed_local),
                        expr: Expr::Box(ValType::FuncRef, func_ref_local),
                    },
                    ExprAssign {
                        local: None,
                        expr: Expr::VectorSet(vector_local, index_local, boxed_local),
                    },
                ],
                next: BasicBlockNext::Jump(BasicBlockId::from(1)),
            });

            /*
            bbX_ref = get_global bbX_ref_global
            if bbX_ref[0] != bbX_ref[index]
                bbX_ref[index] = box(stub_func)
            */

            for &(bb_id, index, stub_func_id) in required_stubs.iter() {
                let cond_bb_id = BasicBlockId::from(bbs.len());
                let then_bb_id = BasicBlockId::from(bbs.len() + 1);
                let next_bb_id = BasicBlockId::from(bbs.len() + 2);

                bbs.push(BasicBlock {
                    id: cond_bb_id,
                    exprs: vec![
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::GlobalGet(self.jit_bbs[bb_id].global),
                        },
                        ExprAssign {
                            local: Some(vector_local),
                            expr: Expr::Unbox(ValType::Vector, boxed_local),
                        },
                        ExprAssign {
                            local: Some(index_local),
                            expr: Expr::Int(GLOBAL_LAYOUT_DEFAULT_INDEX as i64),
                        },
                        ExprAssign {
                            local: Some(boxed_stub0_local),
                            expr: Expr::VectorRef(vector_local, index_local),
                        },
                        ExprAssign {
                            local: Some(index_local),
                            expr: Expr::Int(index as i64),
                        },
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::VectorRef(vector_local, index_local),
                        },
                        ExprAssign {
                            local: Some(bool_local),
                            expr: Expr::Eq(boxed_stub0_local, boxed_local),
                        },
                    ],
                    next: BasicBlockNext::If(bool_local, then_bb_id, next_bb_id),
                });
                bbs.push(BasicBlock {
                    id: then_bb_id,
                    exprs: vec![
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::FuncRef(stub_func_id),
                        },
                        ExprAssign {
                            local: Some(boxed_local),
                            expr: Expr::Box(ValType::FuncRef, boxed_local),
                        },
                        ExprAssign {
                            local: None,
                            expr: Expr::VectorSet(vector_local, index_local, boxed_local),
                        },
                    ],
                    next: BasicBlockNext::Jump(next_bb_id),
                });
            }

            bbs.push(BasicBlock {
                id: bbs.next_key(),
                exprs: vec![],
                next: BasicBlockNext::Return(func_ref_local),
            });

            Func {
                id: funcs.next_key(),
                args: vec![],
                ret_type: LocalType::Type(Type::Val(ValType::FuncRef)), // TODO: Nilでも返したほうがよさそう
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs,
                jit_strategy: FuncJitStrategy::Never,
            }
        };
        let entry_func_id = entry_func.id;
        funcs.push(entry_func);

        Module {
            globals: self.globals.clone(),
            funcs,
            entry: entry_func_id,
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

    results
}

#[derive(Debug, Clone, Default)]
struct BBInfo {
    args: Vec<LocalId>,
    defines: Vec<LocalId>,
    type_params: FxBiHashMap<TypeParamId, LocalId>,
    to_original_locals_mapping: TiVec<LocalId, LocalId>,
    from_original_locals_mapping: FxHashMap<LocalId, LocalId>,
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
                    && let Some(typ) = type_args[type_param_id]
                {
                    LocalType::Type(Type::Val(typ))
                } else {
                    func.locals[self.to_original_locals_mapping[arg]]
                }
            })
            .collect::<Vec<_>>()
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
            if let LocalType::Type(Type::Boxed) = locals[to_original_locals_mapping[arg]] {
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
            type_params: type_params.into_iter_enumerated().collect(),
            to_original_locals_mapping,
            from_original_locals_mapping,
        };

        bb_info.push(info);
    }

    bb_info
}

fn calculate_args_to_pass(
    caller: &BBInfo,
    callee: &BBInfo,
    caller_next_type_args: &TiVec<LocalId, Option<NextTypeArg>>,
) -> (Vec<LocalId>, TiVec<TypeParamId, Option<ValType>>) {
    let mut type_args = ti_vec![None; callee.type_params.len()];
    let mut args_to_pass = Vec::new();

    for &callee_arg in &callee.args {
        let caller_args =
            caller.from_original_locals_mapping[&callee.to_original_locals_mapping[callee_arg]];
        let caller_args = if let Some(&type_param_id) = callee.type_params.get_by_right(&callee_arg)
            && let Some(caller_next_type_arg) = caller_next_type_args[caller_args]
        {
            type_args[type_param_id] = Some(caller_next_type_arg.typ);
            caller_next_type_arg.unboxed
        } else {
            caller_args
        };

        args_to_pass.push(caller_args);
    }
    (args_to_pass, type_args)
}

pub const GLOBAL_LAYOUT_MAX_SIZE: usize = 32;
pub const GLOBAL_LAYOUT_DEFAULT_INDEX: usize = 0;

#[derive(Debug)]
pub struct GlobalLayout {
    type_params_to_index: FxBiHashMap<TiVec<TypeParamId, Option<ValType>>, usize>,
}

impl Default for GlobalLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalLayout {
    pub fn new() -> Self {
        let mut type_params_to_index = FxBiHashMap::default();
        type_params_to_index.insert(ti_vec![], GLOBAL_LAYOUT_DEFAULT_INDEX); // 全ての型パラメータがNoneの時に対応
        Self {
            type_params_to_index,
        }
    }

    pub fn to_idx(&mut self, type_params: &TiVec<TypeParamId, Option<ValType>>) -> usize {
        if type_params.iter().all(|t| t.is_none()) {
            // 全ての型パラメータがNoneなら0を返す
            GLOBAL_LAYOUT_DEFAULT_INDEX
        } else if let Some(&index) = self.type_params_to_index.get_by_left(type_params) {
            index
        } else if self.type_params_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.type_params_to_index.len();
            self.type_params_to_index.insert(type_params.clone(), index);
            index
        } else {
            GLOBAL_LAYOUT_DEFAULT_INDEX
        }
    }

    pub fn from_idx(
        &self,
        index: usize,
        params_len: usize,
    ) -> Cow<TiVec<TypeParamId, Option<ValType>>> {
        if index == GLOBAL_LAYOUT_DEFAULT_INDEX {
            Cow::Owned(ti_vec![None; params_len])
        } else {
            Cow::Borrowed(self.type_params_to_index.get_by_right(&index).unwrap())
        }
    }
}
