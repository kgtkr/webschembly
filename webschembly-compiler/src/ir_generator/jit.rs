use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::{TiVec, ti_vec};

use crate::ir::*;
use crate::ir_generator::GlobalManager;

#[derive(Debug)]
pub struct Jit {
    modules: TiVec<ModuleId, Module>,
    jit_module: TiVec<ModuleId, JitModule>,
}

impl Jit {
    pub fn new() -> Self {
        Self {
            modules: TiVec::new(),
            jit_module: TiVec::new(),
        }
    }

    pub fn register_module(
        &mut self,
        global_manager: &mut GlobalManager,
        module: Module,
    ) -> Module {
        let module_id = self.modules.push_and_get_key(module);
        self.jit_module
            .push(JitModule::new(global_manager, self, module_id));
        self.jit_module[module_id].generate_stub_module(self)
    }

    pub fn instantiate_func(&self, module_id: ModuleId, func_id: FuncId) -> Module {
        let jit_module = &self.jit_module[module_id];
        jit_module
            .generate_jit_func(self, func_id)
            .generate_stub_module(self, jit_module)
    }
}

#[derive(Debug)]
struct JitModule {
    module_id: ModuleId,
    func_ref_globals: TiVec<FuncId, GlobalId>,
    globals: FxHashSet<GlobalId>,
}

impl JitModule {
    fn new(global_manager: &mut GlobalManager, jit: &Jit, module_id: ModuleId) -> Self {
        let module = &jit.modules[module_id];
        let func_ref_globals = module
            .funcs
            .iter()
            .map(|_| global_manager.gen_global_id())
            .collect::<TiVec<FuncId, _>>();

        let globals = {
            let mut globals = module.globals.clone();
            globals.extend(func_ref_globals.iter());
            globals
        };

        Self {
            module_id,
            func_ref_globals,
            globals,
        }
    }

    fn generate_stub_module(&self, jit: &Jit) -> Module {
        let module = &jit.modules[self.module_id];
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
                    for func in module.funcs.iter() {
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
                            expr: Expr::GlobalSet(self.func_ref_globals[func.id], LocalId::from(2)),
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
                            expr: Expr::InstantiateFunc(self.module_id, func.id),
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

    fn generate_jit_func(&self, jit: &Jit, func_id: FuncId) -> JitFunc {
        JitFunc::new(&mut GlobalManager::new(), jit, self, func_id)
    }
}

#[derive(Debug)]
struct JitFunc {
    func_id: FuncId,
    func_ref_globals: TiVec<BasicBlockId, GlobalId>,
    bb_infos: TiVec<BasicBlockId, BBInfo>,
    globals: FxHashSet<GlobalId>,
}

impl JitFunc {
    fn new(
        global_manager: &mut GlobalManager,
        jit: &Jit,
        jit_module: &JitModule,
        func_id: FuncId,
    ) -> Self {
        let module = &jit.modules[jit_module.module_id];
        let func = &module.funcs[func_id];
        let func_ref_globals = func
            .bbs
            .iter()
            .map(|_| global_manager.gen_global_id())
            .collect::<TiVec<BasicBlockId, _>>();

        let bb_infos = calculate_bb_info(analyze_locals(func));

        let globals = {
            let mut globals = jit_module.globals.clone();
            globals.extend(func_ref_globals.iter());
            globals
        };

        Self {
            func_id,
            func_ref_globals,
            bb_infos,
            globals,
        }
    }

    fn generate_stub_module(&self, jit: &Jit, jit_module: &JitModule) -> Module {
        let module = &jit.modules[jit_module.module_id];
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
        let func = &module.funcs[self.func_id];

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
                        expr: Expr::GlobalSet(
                            jit_module.func_ref_globals[func.id],
                            LocalId::from(1)
                        ),
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
                let mut locals = func.locals.clone();
                locals.push(LocalType::Type(Type::Boxed));
                locals.push(LocalType::Type(Type::Val(ValType::FuncRef)));
                locals
            },
            bb_entry: func.bb_entry,
            bbs: func
                .bbs
                .iter()
                .map(|bb| {
                    let mut exprs = Vec::new();
                    for expr in bb.exprs.iter() {
                        // FuncRefとCall命令はget global命令に置き換えられる
                        match &expr.expr {
                            Expr::FuncRef(id) => {
                                exprs.push(ExprAssign {
                                    local: Some(boxed_func_ref),
                                    expr: Expr::GlobalGet(jit_module.func_ref_globals[*id]),
                                });
                                exprs.push(ExprAssign {
                                    local: expr.local,
                                    expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                                });
                            }
                            Expr::Call(ExprCall { func_id, args }) => {
                                exprs.push(ExprAssign {
                                    local: Some(boxed_func_ref),
                                    expr: Expr::GlobalGet(jit_module.func_ref_globals[*func_id]),
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
                                        func_type: module.funcs[*func_id].func_type(),
                                    }),
                                });
                            }
                            _ => {
                                exprs.push(expr.clone());
                            }
                        }
                    }

                    let next = match &bb.next {
                        BasicBlockNext::TailCall(ExprCall { func_id, args }) => {
                            exprs.push(ExprAssign {
                                local: Some(boxed_func_ref),
                                expr: Expr::GlobalGet(jit_module.func_ref_globals[*func_id]),
                            });
                            exprs.push(ExprAssign {
                                local: Some(func_ref),
                                expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                            });
                            BasicBlockNext::TailCallRef(ExprCallRef {
                                func: func_ref,
                                args: args.clone(),
                                func_type: module.funcs[*func_id].func_type(),
                            })
                        }
                        next @ (BasicBlockNext::TailCallRef(_)
                        | BasicBlockNext::Return(_)
                        | BasicBlockNext::If(_, _, _)
                        | BasicBlockNext::Jump(_)) => next.clone(),
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

    /*  fn generate_bb_module(&self, jit: &Jit, bb_id: BasicBlockId) -> Module {
        let module = &jit.modules[self.module_id];
        let func = &module.funcs[self.func_id];
        let bb = &func.bbs[bb_id];
        let bb_info = &self.bb_infos[bb_id];

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

        for func_id in bb.func_ids_mut() {
            let new_target_func_id = new_func_ids[func_id];
            *func_id = new_target_func_id;
        }
        let mut extra_bbs = Vec::new();

        let new_next = match bb.next {
            BasicBlockNext::If(cond, then_bb, else_bb) => {
                let then_func_id = bb_to_func_id[&(orig_func.id, then_bb)];
                let else_func_id = bb_to_func_id[&(orig_func.id, else_bb)];

                let then_locals_to_pass =
                    calculate_args_to_pass(bb_info, &bb_infos[orig_func.id][then_bb]);
                let else_locals_to_pass =
                    calculate_args_to_pass(bb_info, &bb_infos[orig_func.id][else_bb]);

                let then_bb_new = BasicBlock {
                    id: BasicBlockId::from(1),
                    exprs: vec![],
                    next: BasicBlockNext::TailCall(ExprCall {
                        func_id: then_func_id,
                        args: then_locals_to_pass,
                    }),
                };

                let else_bb_new = BasicBlock {
                    id: BasicBlockId::from(2),
                    exprs: vec![],
                    next: BasicBlockNext::TailCall(ExprCall {
                        func_id: else_func_id,
                        args: else_locals_to_pass,
                    }),
                };

                extra_bbs.push(then_bb_new);
                extra_bbs.push(else_bb_new);

                BasicBlockNext::If(cond, BasicBlockId::from(1), BasicBlockId::from(2))
            }
            BasicBlockNext::Jump(target_bb) => {
                let target_func_id = bb_to_func_id[&(orig_func.id, target_bb)];
                let args_to_pass =
                    calculate_args_to_pass(bb_info, &bb_infos[orig_func.id][target_bb]);

                BasicBlockNext::TailCall(ExprCall {
                    func_id: target_func_id,
                    args: args_to_pass,
                })
            }
            next @ (BasicBlockNext::Return(_)
            | BasicBlockNext::TailCall(_)
            | BasicBlockNext::TailCallRef(_)) => next,
        };

        let new_bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: bb.exprs,
            next: new_next,
        };

        let mut new_bbs = TiVec::new();
        new_bbs.push(new_bb);
        new_bbs.extend(extra_bbs.into_iter());

        let new_func = Func {
            id: new_func_id,
            locals: new_locals,
            args: bb_info.args.len(),
            ret_type: orig_func.ret_type,
            bb_entry: BasicBlockId::from(0),
            bbs: new_bbs,
            jit_strategy: if orig_func.bb_entry == bb.id {
                orig_func.jit_strategy
            } else {
                FuncJitStrategy::BasicBlock
            },
        };

        new_funcs.push(new_func);
    }*/
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
