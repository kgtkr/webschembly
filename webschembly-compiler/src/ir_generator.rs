use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::*;
use crate::{
    ast::{self, Desugared, TailCall, Used},
    x::{RunX, TypeMap, type_map},
};
use typed_index_collections::{TiVec, ti_vec};

#[derive(Debug, Clone)]
pub struct Config {
    pub allow_set_builtin: bool,
}

#[derive(Debug, Clone)]
struct BasicBlockOptionalNext {
    pub exprs: Vec<ExprAssign>,
    pub next: Option<BasicBlockNext>,
}

#[derive(Debug)]
pub struct IrGenerator {
    modules: TiVec<ModuleId, Module>,
    global_count: usize,
    // GlobalIdのうち、ast::GlobalVarIdに対応するもの
    // 全てのGlobalIdがast::GlobalVarIdに対応するわけではない
    global_ids: FxHashMap<ast::GlobalVarId, GlobalId>,
}

impl Default for IrGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl IrGenerator {
    pub fn new() -> Self {
        Self {
            modules: TiVec::new(),
            global_count: 0,
            global_ids: FxHashMap::default(),
        }
    }

    pub fn register_module(&mut self, module: Module) -> ModuleId {
        self.modules.push_and_get_key(module)
    }

    pub fn generate_module(&mut self, ast: &ast::Ast<ast::Final>, config: Config) -> Module {
        let module_gen = ModuleGenerator::new(config, self, ast);
        let module = module_gen.generate();
        module
    }

    pub fn get_module(&self, id: ModuleId) -> &Module {
        self.modules.get(id).unwrap()
    }

    pub fn gen_global_id(&mut self) -> GlobalId {
        let id = GlobalId::from(self.global_count);
        self.global_count += 1;
        id
    }

    pub fn split_and_register_module(&mut self, module: Module) -> ModuleId {
        let func_ref_globals = module
            .funcs
            .iter()
            .map(|_| self.gen_global_id())
            .collect::<TiVec<FuncId, _>>();

        // eqのためにstubのfunc_refをcacheする
        // TODO: FuncRefは何度生成しても同じ参照になるようにwasm generatorで対応するべき
        let stub_func_ref_globals = module
            .funcs
            .iter()
            .map(|_| self.gen_global_id())
            .collect::<TiVec<FuncId, _>>();

        let func_types = module
            .funcs
            .iter()
            .map(|func| func.func_type())
            .collect::<TiVec<FuncId, _>>();

        let global_count = self.global_count;
        let module_ids = module
            .funcs
            .iter()
            .enumerate()
            .map(|(i, _)| ModuleId::from(1 + i + self.modules.len()))
            .collect::<TiVec<FuncId, _>>();
        // エントリーモジュール
        let entry_module = {
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
                ret: LocalId::from(0),
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
                            expr: Expr::InitGlobals(global_count),
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
                                    func_ref_globals[func_id.id],
                                    LocalId::from(2),
                                ),
                            });
                            exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::GlobalSet(
                                    stub_func_ref_globals[func_id.id],
                                    LocalId::from(2),
                                ),
                            });
                        }
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(0)),
                            expr: Expr::Call(true, stub_func_ids[module.entry], vec![]),
                        });
                        exprs
                    },
                    next: BasicBlockNext::Return,
                },],
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
                    ret: LocalId::from(func.args + 0),
                    locals: {
                        let mut locals = TiVec::new();
                        locals.extend(func.arg_types().into_iter().map(LocalType::Type));
                        locals.extend(vec![
                            LocalType::Type(func.ret_type()),
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
                            exprs: {
                                let mut exprs = Vec::new();
                                exprs.push(ExprAssign {
                                    local: Some(LocalId::from(func.args + 1)),
                                    expr: Expr::GlobalGet(func_ref_globals[func.id]),
                                });
                                exprs.push(ExprAssign {
                                    local: Some(LocalId::from(func.args + 3)),
                                    expr: Expr::GlobalGet(stub_func_ref_globals[func.id]),
                                });
                                exprs.push(ExprAssign {
                                    local: Some(LocalId::from(func.args + 4)),
                                    expr: Expr::Eq(
                                        LocalId::from(func.args + 1),
                                        LocalId::from(func.args + 3),
                                    ),
                                });
                                exprs
                            },
                            next: BasicBlockNext::If(
                                LocalId::from(func.args + 4),
                                BasicBlockId::from(1),
                                BasicBlockId::from(2),
                            ),
                        },
                        BasicBlock {
                            id: BasicBlockId::from(1),
                            exprs: {
                                let mut exprs = Vec::new();
                                exprs.push(ExprAssign {
                                    local: None,
                                    expr: Expr::InstantiateModule(module_ids[func.id]),
                                });
                                exprs
                            },
                            next: BasicBlockNext::Jump(BasicBlockId::from(2)),
                        },
                        BasicBlock {
                            id: BasicBlockId::from(2),
                            exprs: {
                                let mut exprs = Vec::new();
                                exprs.push(ExprAssign {
                                    local: Some(LocalId::from(func.args + 1)),
                                    expr: Expr::GlobalGet(func_ref_globals[func.id]),
                                });
                                exprs.push(ExprAssign {
                                    local: Some(LocalId::from(func.args + 2)),
                                    expr: Expr::Unbox(
                                        ValType::FuncRef,
                                        LocalId::from(func.args + 1),
                                    ),
                                });
                                exprs.push(ExprAssign {
                                    local: Some(LocalId::from(func.args + 0)),
                                    expr: Expr::CallRef(
                                        true,
                                        LocalId::from(func.args + 2),
                                        (0..func.args)
                                            .map(|i| LocalId::from(i))
                                            .collect::<Vec<_>>(),
                                        func.func_type(),
                                    ),
                                });
                                exprs
                            },
                            next: BasicBlockNext::Return,
                        },
                    ],
                };
                funcs.push(func);
            }

            Module {
                funcs,
                entry: FuncId::from(0),
                meta: Meta {
                    // TODO:
                    local_metas: FxHashMap::default(),
                    global_metas: FxHashMap::default(),
                },
            }
        };

        let entry_module_id = self.modules.next_key();
        self.modules.push(entry_module);

        // 各関数のモジュール
        for func in module.funcs {
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
                ret: LocalId::from(0),
                locals: ti_vec![
                    LocalType::Type(Type::Val(ValType::FuncRef)),
                    LocalType::Type(Type::Boxed)
                ],
                bb_entry: BasicBlockId::from(0),
                bbs: ti_vec![BasicBlock {
                    id: BasicBlockId::from(0),
                    exprs: {
                        let mut exprs = Vec::new();
                        exprs.push(ExprAssign {
                            local: None,
                            expr: Expr::InitGlobals(global_count),
                        });
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(0)),
                            expr: Expr::FuncRef(FuncId::from(1)),
                        });
                        exprs.push(ExprAssign {
                            local: Some(LocalId::from(1)),
                            expr: Expr::Box(ValType::FuncRef, LocalId::from(0)),
                        });
                        exprs.push(ExprAssign {
                            local: None,
                            expr: Expr::GlobalSet(func_ref_globals[func.id], LocalId::from(1)),
                        });
                        exprs
                    },
                    next: BasicBlockNext::Return,
                },],
            };
            funcs.push(entry_func);
            let boxed_func_ref = func.locals.next_key();
            let func_ref = LocalId::from(usize::from(boxed_func_ref) + 1);
            let body_func = Func {
                id: funcs.next_key(),
                args: func.args,
                ret: func.ret,
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
                    .map(|bb| BasicBlock {
                        id: bb.id,
                        exprs: {
                            let mut exprs = Vec::new();
                            for expr in bb.exprs {
                                // FuncRefとCall命令はget global命令に置き換えられる
                                match expr.expr {
                                    Expr::FuncRef(id) => {
                                        exprs.push(ExprAssign {
                                            local: Some(boxed_func_ref),
                                            expr: Expr::GlobalGet(func_ref_globals[id]),
                                        });
                                        exprs.push(ExprAssign {
                                            local: expr.local,
                                            expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                                        });
                                    }
                                    Expr::Call(tail_call, id, args) => {
                                        exprs.push(ExprAssign {
                                            local: Some(boxed_func_ref),
                                            expr: Expr::GlobalGet(func_ref_globals[id]),
                                        });
                                        exprs.push(ExprAssign {
                                            local: Some(func_ref),
                                            expr: Expr::Unbox(ValType::FuncRef, boxed_func_ref),
                                        });
                                        exprs.push(ExprAssign {
                                            local: expr.local,
                                            expr: Expr::CallRef(
                                                tail_call,
                                                func_ref,
                                                args,
                                                func_types[id].clone(),
                                            ),
                                        });
                                    }
                                    _ => {
                                        exprs.push(expr);
                                    }
                                }
                            }
                            exprs
                        },
                        next: bb.next,
                    })
                    .collect(),
            };

            funcs.push(body_func);

            let module = Module {
                funcs,
                entry: FuncId::from(0),
                meta: Meta {
                    // TODO:
                    local_metas: FxHashMap::default(),
                    global_metas: FxHashMap::default(),
                },
            };

            self.modules.push(module);
        }

        entry_module_id
    }
}

#[derive(Debug)]
struct ModuleGenerator<'a> {
    ir_generator: &'a mut IrGenerator,
    ast: &'a ast::Ast<ast::Final>,
    funcs: TiVec<FuncId, Option<Func>>,
    config: Config,
    // メタ情報
    local_metas: FxHashMap<(FuncId, LocalId), VarMeta>,
    global_metas: FxHashMap<GlobalId, VarMeta>,
}

impl<'a> ModuleGenerator<'a> {
    fn new(
        config: Config,
        ir_generator: &'a mut IrGenerator,
        ast: &'a ast::Ast<ast::Final>,
    ) -> Self {
        Self {
            ast,
            ir_generator,
            funcs: TiVec::new(),
            config,
            local_metas: FxHashMap::default(),
            global_metas: FxHashMap::default(),
        }
    }

    fn generate(mut self) -> Module {
        let func_id = self.funcs.push_and_get_key(None);
        let func = FuncGenerator::new(&mut self, func_id).entry_gen();
        self.funcs[func_id] = Some(func);

        let meta = Meta {
            local_metas: self.local_metas,
            global_metas: self.global_metas,
        };
        Module {
            funcs: self.funcs.into_iter().map(|f| f.unwrap()).collect(),
            entry: func_id,
            meta,
        }
    }

    fn gen_func(
        &mut self,
        x: &RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> (FuncId, FuncId) {
        let id = self.funcs.push_and_get_key(None);
        let func = FuncGenerator::new(self, id).lambda_gen(x, lambda);
        self.funcs[id] = Some(func);

        let boxed_id = self.funcs.push_and_get_key(None);
        let boxed_func = FuncGenerator::new(self, boxed_id).boxed_func_gen(id, lambda.args.len());
        self.funcs[boxed_id] = Some(boxed_func);

        (id, boxed_id)
    }
}

#[derive(Debug)]
struct FuncGenerator<'a, 'b> {
    id: FuncId,
    locals: TiVec<LocalId, LocalType>,
    local_ids: FxHashMap<ast::LocalVarId, LocalId>,
    bbs: TiVec<BasicBlockId, BasicBlockOptionalNext>,
    next_undecided_bb_ids: FxHashSet<BasicBlockId>,
    module_generator: &'a mut ModuleGenerator<'b>,
    exprs: Vec<ExprAssign>,
}

impl<'a, 'b> FuncGenerator<'a, 'b> {
    fn new(module_generator: &'a mut ModuleGenerator<'b>, id: FuncId) -> Self {
        Self {
            id,
            locals: TiVec::new(),
            local_ids: FxHashMap::default(),
            bbs: TiVec::new(),
            next_undecided_bb_ids: FxHashSet::default(),
            module_generator,
            exprs: Vec::new(),
        }
    }

    fn entry_gen(mut self) -> Func {
        let boxed_local = self.local(Type::Boxed);

        self.exprs.push(ExprAssign {
            local: None,
            expr: Expr::InitGlobals(
                self.module_generator
                    .ast
                    .x
                    .get_ref(type_map::key::<Used>())
                    .global_vars
                    .iter()
                    .map(|x| x.0)
                    .max()
                    .map(|n| n + 1)
                    .unwrap_or(0),
            ),
        });
        self.gen_exprs(Some(boxed_local), &self.module_generator.ast.exprs);
        self.close_bb(Some(BasicBlockNext::Return));
        Func {
            id: self.id,
            args: 0,
            ret: boxed_local,
            locals: self.locals,
            bb_entry: BasicBlockId::from(0), // TODO: もっと綺麗な書き方があるはず
            bbs: self
                .bbs
                .into_iter_enumerated()
                .map(|(id, bb)| BasicBlock {
                    id,
                    exprs: bb.exprs,
                    next: bb.next.unwrap(),
                })
                .collect(),
        }
    }

    fn lambda_gen(
        mut self,
        x: &RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> Func {
        let self_closure = self.local(Type::Val(ValType::Closure));
        let mut arg_locals = Vec::new();
        for _ in &x.get_ref(type_map::key::<Used>()).args {
            // 引数にMutCellは使えないので一旦全てBoxedで定義
            let arg_local = self.local(Type::Boxed);
            arg_locals.push(arg_local);
        }
        for (arg_local, arg) in arg_locals
            .into_iter()
            .zip(&x.get_ref(type_map::key::<Used>()).args)
        {
            let local = self.define_ast_local(*arg);
            if self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .box_vars
                .contains(arg)
            {
                self.exprs.push(ExprAssign {
                    local: Some(local),
                    expr: Expr::CreateMutCell(Type::Boxed),
                });

                self.exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::SetMutCell(Type::Boxed, local, arg_local),
                });
            } else {
                self.exprs.push(ExprAssign {
                    local: Some(local),
                    expr: Expr::Move(arg_local),
                });
            }
        }

        // 環境を復元するためのローカル変数を定義
        for var_id in x.get_ref(type_map::key::<Used>()).captures.iter() {
            self.define_ast_local(*var_id);
        }
        // 環境の型を収集
        let env_types = x
            .get_ref(type_map::key::<Used>())
            .captures
            .iter()
            .map(|id| self.locals[*self.local_ids.get(id).unwrap()])
            .collect::<Vec<_>>();
        // 環境を復元する処理を追加
        for (i, var_id) in x
            .get_ref(type_map::key::<Used>())
            .captures
            .iter()
            .enumerate()
        {
            let env_local = *self.local_ids.get(var_id).unwrap();
            self.exprs.push(ExprAssign {
                local: Some(env_local),
                // TODO: 無駄なclone。Irの設計を見直す
                expr: Expr::ClosureEnv(env_types.clone(), self_closure, i),
            });
        }

        for id in &x.get_ref(type_map::key::<Used>()).defines {
            let local = self.define_ast_local(*id);
            if self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .box_vars
                .contains(id)
            {
                self.exprs.push(ExprAssign {
                    local: Some(local),
                    expr: Expr::CreateMutCell(Type::Boxed),
                });
            }
        }

        let ret = self.local(Type::Boxed);
        self.gen_exprs(Some(ret), &lambda.body);
        self.close_bb(Some(BasicBlockNext::Return));
        Func {
            id: self.id,
            args: lambda.args.len() + 1,
            ret,
            locals: self.locals,
            bb_entry: BasicBlockId::from(0), // TODO: もっと綺麗な書き方があるはず
            bbs: self
                .bbs
                .into_iter_enumerated()
                .map(|(id, bb)| BasicBlock {
                    id,
                    exprs: bb.exprs,
                    next: bb.next.unwrap(),
                })
                .collect(),
        }
    }

    fn boxed_func_gen(mut self, target_func_id: FuncId, args_len: usize) -> Func {
        let self_closure = self.local(Type::Val(ValType::Closure));
        let vector = self.local(Type::Val(ValType::Vector));
        let mut args = Vec::new();
        args.push(self_closure);
        for i in 0..args_len {
            let arg = self.local(Type::Boxed);
            let arg_i = self.local(Type::Val(ValType::Int));
            self.exprs.push(ExprAssign {
                local: Some(arg_i),
                expr: Expr::Int(i as i64),
            });
            self.exprs.push(ExprAssign {
                local: Some(arg),
                expr: Expr::VectorRef(vector, arg_i),
            });
            args.push(arg);
        }
        let ret = self.local(Type::Boxed);
        self.exprs.push(ExprAssign {
            local: Some(ret),
            expr: Expr::Call(true, target_func_id, args),
        });
        self.close_bb(Some(BasicBlockNext::Return));
        Func {
            id: self.id,
            args: 2,
            ret,
            locals: self.locals,
            bb_entry: BasicBlockId::from(0),
            bbs: self
                .bbs
                .into_iter_enumerated()
                .map(|(id, bb)| BasicBlock {
                    id,
                    exprs: bb.exprs,
                    next: bb.next.unwrap(),
                })
                .collect(),
        }
    }

    fn local<T: Into<LocalType>>(&mut self, typ: T) -> LocalId {
        self.locals.push_and_get_key(typ.into())
    }

    fn define_ast_local(&mut self, id: ast::LocalVarId) -> LocalId {
        let ast_meta = self
            .module_generator
            .ast
            .x
            .get_ref(type_map::key::<Used>())
            .local_metas
            .get(&id);
        let local = self.local(
            if self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .box_vars
                .contains(&id)
            {
                LocalType::MutCell(Type::Boxed)
            } else {
                LocalType::Type(Type::Boxed)
            },
        );
        self.local_ids.insert(id, local);
        if let Some(ast_meta) = ast_meta {
            self.module_generator
                .local_metas
                .insert((self.id, local), VarMeta {
                    name: ast_meta.name.clone(),
                });
        }
        local
    }

    fn gen_expr(&mut self, result: Option<LocalId>, ast: &ast::Expr<ast::Final>) {
        match ast {
            ast::Expr::Const(_, lit) => match lit {
                ast::Const::Bool(b) => {
                    let unboxed = self.local(Type::Val(ValType::Bool));
                    self.exprs.push(ExprAssign {
                        local: Some(unboxed),
                        expr: Expr::Bool(*b),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::Box(ValType::Bool, unboxed),
                    });
                }
                ast::Const::Int(i) => {
                    let unboxed = self.local(Type::Val(ValType::Int));
                    self.exprs.push(ExprAssign {
                        local: Some(unboxed),
                        expr: Expr::Int(*i),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::Box(ValType::Int, unboxed),
                    });
                }
                ast::Const::String(s) => {
                    let unboxed = self.local(Type::Val(ValType::String));
                    self.exprs.push(ExprAssign {
                        local: Some(unboxed),
                        expr: Expr::String(s.clone()),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::Box(ValType::String, unboxed),
                    });
                }
                ast::Const::Nil => {
                    let unboxed = self.local(Type::Val(ValType::Nil));
                    self.exprs.push(ExprAssign {
                        local: Some(unboxed),
                        expr: Expr::Nil,
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::Box(ValType::Nil, unboxed),
                    });
                }
                ast::Const::Char(c) => {
                    let unboxed = self.local(Type::Val(ValType::Char));
                    self.exprs.push(ExprAssign {
                        local: Some(unboxed),
                        expr: Expr::Char(*c),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::Box(ValType::Char, unboxed),
                    });
                }
                ast::Const::Symbol(s) => {
                    let string = self.local(Type::Val(ValType::String));
                    let unboxed = self.local(Type::Val(ValType::Symbol));
                    self.exprs.push(ExprAssign {
                        local: Some(string),
                        expr: Expr::String(s.clone()),
                    });
                    self.exprs.push(ExprAssign {
                        local: Some(unboxed),
                        expr: Expr::StringToSymbol(string),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::Box(ValType::Symbol, unboxed),
                    });
                }
            },
            ast::Expr::Define(x, _) => *x.get_ref(type_map::key::<Used>()),
            ast::Expr::Lambda(x, lambda) => {
                let captures = x
                    .get_ref(type_map::key::<Used>())
                    .captures
                    .iter()
                    .map(|id| *self.local_ids.get(id).unwrap())
                    .collect::<Vec<_>>();
                let (func_id, boxed_func_id) = self.module_generator.gen_func(x, lambda);
                let func_local = self.local(ValType::FuncRef);
                let boxed_func_local = self.local(ValType::FuncRef);
                let unboxed = self.local(Type::Val(ValType::Closure));
                self.exprs.push(ExprAssign {
                    local: Some(func_local),
                    expr: Expr::FuncRef(func_id),
                });
                self.exprs.push(ExprAssign {
                    local: Some(boxed_func_local),
                    expr: Expr::FuncRef(boxed_func_id),
                });
                self.exprs.push(ExprAssign {
                    local: Some(unboxed),
                    expr: Expr::Closure {
                        envs: captures,
                        func: func_local,
                        boxed_func: boxed_func_local,
                    },
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::Box(ValType::Closure, unboxed),
                });
            }
            ast::Expr::If(_, ast::If { cond, then, els }) => {
                let boxed_cond_local = self.local(Type::Boxed);
                self.gen_expr(Some(boxed_cond_local), cond);

                // TODO: condがboolかのチェック
                let cond_local = self.local(Type::Val(ValType::Bool));
                self.exprs.push(ExprAssign {
                    local: Some(cond_local),
                    expr: Expr::Unbox(ValType::Bool, boxed_cond_local),
                });

                let bb_id = self.close_bb(None);

                let then_first_bb_id = self.bbs.next_key();
                self.gen_expr(result, then);
                let then_last_bb_id = self.close_bb(None);

                let else_first_bb_id = self.bbs.next_key();
                self.gen_expr(result, els);
                let else_last_bb_id = self.close_bb(None);

                self.bbs[bb_id].next = Some(BasicBlockNext::If(
                    cond_local,
                    then_first_bb_id,
                    else_first_bb_id,
                ));

                self.next_undecided_bb_ids.insert(then_last_bb_id);
                self.next_undecided_bb_ids.insert(else_last_bb_id);
            }
            ast::Expr::Call(x, ast::Call { func, args }) => {
                if let ast::Expr::Var(x, name) = func.as_ref()
                    && let ast::UsedVarR {
                        var_id: ast::VarId::Global(_),
                    } = x.get_ref(type_map::key::<Used>())
                    && let Some(builtin) = ast::Builtin::from_name(name)
                {
                    let rule = BuiltinConversionRule::from_builtin(builtin);
                    if rule.arg_count() != args.len() {
                        let msg = self.local(Type::Val(ValType::String));
                        self.exprs.push(ExprAssign {
                            local: Some(msg),
                            expr: Expr::String("builtin args count mismatch\n".to_string()),
                        });
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::Error(msg),
                        });
                    } else {
                        let mut arg_locals = Vec::new();
                        for (typ, arg) in rule.arg_types().iter().zip(args) {
                            let boxed_arg_local = self.local(Type::Boxed);
                            self.gen_expr(Some(boxed_arg_local), arg);
                            let arg_local = match typ {
                                Type::Boxed => boxed_arg_local,
                                Type::Val(val_type) => {
                                    let unboxed_arg_local = self.local(Type::Val(*val_type));
                                    // TODO: 動的型チェック
                                    self.exprs.push(ExprAssign {
                                        local: Some(unboxed_arg_local),
                                        expr: Expr::Unbox(*val_type, boxed_arg_local),
                                    });
                                    unboxed_arg_local
                                }
                            };
                            arg_locals.push(arg_local);
                        }

                        let ret_local = match rule.ret_type() {
                            Type::Boxed => self.local(Type::Boxed),
                            Type::Val(val_type) => self.local(Type::Val(val_type)),
                        };
                        let expr = match rule {
                            BuiltinConversionRule::Unary { to_ir, .. } => to_ir(arg_locals[0]),
                            BuiltinConversionRule::Binary { to_ir, .. } => {
                                to_ir(arg_locals[0], arg_locals[1])
                            }
                            BuiltinConversionRule::Ternary { to_ir, .. } => {
                                to_ir(arg_locals[0], arg_locals[1], arg_locals[2])
                            }
                        };
                        self.exprs.push(ExprAssign {
                            local: Some(ret_local),
                            expr,
                        });
                        match rule.ret_type() {
                            Type::Boxed => {
                                self.exprs.push(ExprAssign {
                                    local: result,
                                    expr: Expr::Move(ret_local),
                                });
                            }
                            Type::Val(val_type) => {
                                self.exprs.push(ExprAssign {
                                    local: result,
                                    expr: Expr::Box(val_type, ret_local),
                                });
                            }
                        }
                    }
                } else {
                    let boxed_func_local = self.local(Type::Boxed);
                    self.gen_expr(Some(boxed_func_local), func);

                    // TODO: funcがクロージャかのチェック
                    let closure_local = self.local(ValType::Closure);
                    let func_local = self.local(ValType::FuncRef);
                    self.exprs.push(ExprAssign {
                        local: Some(closure_local),
                        expr: Expr::Unbox(ValType::Closure, boxed_func_local),
                    });
                    self.exprs.push(ExprAssign {
                        local: Some(func_local),
                        expr: Expr::ClosureFuncRef(closure_local),
                    });
                    // TODO: 引数の数が合っているかのチェック
                    let mut arg_locals = Vec::new();
                    let mut args_types = Vec::new();
                    arg_locals.push(closure_local); // 第一引数にクロージャを渡す
                    args_types.push(Type::Val(ValType::Closure));
                    for arg in args {
                        let arg_local = self.local(Type::Boxed);
                        self.gen_expr(Some(arg_local), arg);
                        arg_locals.push(arg_local);
                        args_types.push(Type::Boxed);
                    }
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::CallRef(
                            x.get_ref(type_map::key::<TailCall>()).is_tail,
                            func_local,
                            arg_locals,
                            FuncType {
                                ret: Type::Boxed,
                                args: args_types,
                            },
                        ),
                    });
                }
            }
            ast::Expr::Var(x, _) => match &x.get_ref(type_map::key::<Used>()).var_id {
                ast::VarId::Local(id) => {
                    if self
                        .module_generator
                        .ast
                        .x
                        .get_ref(type_map::key::<Used>())
                        .box_vars
                        .contains(id)
                    {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::DerefMutCell(Type::Boxed, *self.local_ids.get(id).unwrap()),
                        });
                    } else {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::Move(*self.local_ids.get(id).unwrap()),
                        });
                    }
                }
                ast::VarId::Global(id) => {
                    let global = self.global_id(*id);
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::GlobalGet(global),
                    });
                }
            },
            ast::Expr::Begin(_, ast::Begin { exprs }) => {
                self.gen_exprs(result, exprs);
            }
            ast::Expr::Set(x, ast::Set { name, expr, .. }) => {
                match &x.get_ref(type_map::key::<Used>()).var_id {
                    ast::VarId::Local(id) => {
                        if self
                            .module_generator
                            .ast
                            .x
                            .get_ref(type_map::key::<Used>())
                            .box_vars
                            .contains(id)
                        {
                            let boxed_local = self.local(Type::Boxed);
                            self.gen_expr(Some(boxed_local), expr);
                            let local = self.local_ids.get(id).unwrap();
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::SetMutCell(Type::Boxed, *local, boxed_local),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(boxed_local),
                            });
                        } else {
                            let local = *self.local_ids.get(id).unwrap();
                            self.gen_expr(Some(local), expr);
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(local),
                            });
                        }
                    }
                    ast::VarId::Global(id) => {
                        if let Some(_) = ast::Builtin::from_name(name)
                            && !self.module_generator.config.allow_set_builtin
                        {
                            let msg = self.local(Type::Val(ValType::String));
                            self.exprs.push(ExprAssign {
                                local: Some(msg),
                                expr: Expr::String("set! builtin is not allowed\n".to_string()),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Error(msg),
                            });
                        } else {
                            let local = self.local(Type::Boxed);
                            self.gen_expr(Some(local), expr);
                            let global = self.global_id(*id);
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::GlobalSet(global, local),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(local),
                            });
                        }
                    }
                }
            }
            ast::Expr::Let(x, _) => *x.get_ref(type_map::key::<Desugared>()),
            ast::Expr::Vector(_, vec) => {
                let mut vec_locals = Vec::new();
                for sexpr in vec {
                    let boxed_local = self.local(Type::Boxed);
                    self.gen_expr(Some(boxed_local), sexpr);
                    vec_locals.push(boxed_local);
                }
                let unboxed = self.local(Type::Val(ValType::Vector));
                self.exprs.push(ExprAssign {
                    local: Some(unboxed),
                    expr: Expr::Vector(vec_locals),
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::Box(ValType::Vector, unboxed),
                });
            }
            ast::Expr::Cons(_, cons) => {
                let car_local = self.local(Type::Boxed);
                self.gen_expr(Some(car_local), &cons.car);
                let cdr_local = self.local(Type::Boxed);
                self.gen_expr(Some(cdr_local), &cons.cdr);

                let unboxed = self.local(Type::Val(ValType::Cons));
                self.exprs.push(ExprAssign {
                    local: Some(unboxed),
                    expr: Expr::Cons(car_local, cdr_local),
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::Box(ValType::Cons, unboxed),
                });
            }
            ast::Expr::Quote(x, _) => *x.get_ref(type_map::key::<Desugared>()),
        }
    }

    fn global_id(&mut self, id: ast::GlobalVarId) -> GlobalId {
        if let Some(&global_id) = self.module_generator.ir_generator.global_ids.get(&id) {
            global_id
        } else {
            let ast_meta = self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .global_metas
                .get(&id);
            let global_id = self.module_generator.ir_generator.gen_global_id();
            self.module_generator
                .ir_generator
                .global_ids
                .insert(id, global_id);
            if let Some(ast_meta) = ast_meta {
                self.module_generator
                    .global_metas
                    .insert(global_id, VarMeta {
                        name: ast_meta.name.clone(),
                    });
            }
            global_id
        }
    }

    fn gen_exprs(&mut self, result: Option<LocalId>, exprs: &[ast::Expr<ast::Final>]) {
        if let Some((last, rest)) = exprs.split_last() {
            for expr in rest {
                self.gen_expr(None, expr);
            }
            self.gen_expr(result, last);
        } else {
            let unboxed = self.local(Type::Val(ValType::Nil));
            self.exprs.push(ExprAssign {
                local: Some(unboxed),
                expr: Expr::Nil,
            });
            self.exprs.push(ExprAssign {
                local: result,
                expr: Expr::Box(ValType::Nil, unboxed),
            });
        }
    }

    fn close_bb(&mut self, next: Option<BasicBlockNext>) -> BasicBlockId {
        let bb_exprs = std::mem::take(&mut self.exprs);
        let bb_id = self.bbs.push_and_get_key(BasicBlockOptionalNext {
            exprs: bb_exprs,
            next,
        });

        let undecided_bb_ids = std::mem::take(&mut self.next_undecided_bb_ids);
        for undecided_bb_id in undecided_bb_ids {
            self.bbs[undecided_bb_id].next = Some(BasicBlockNext::Jump(bb_id));
        }
        bb_id
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BuiltinConversionRule {
    Unary {
        args: [Type; 1],
        ret: Type,
        to_ir: fn(LocalId) -> Expr,
    },
    Binary {
        args: [Type; 2],
        ret: Type,
        to_ir: fn(LocalId, LocalId) -> Expr,
    },
    Ternary {
        args: [Type; 3],
        ret: Type,
        to_ir: fn(LocalId, LocalId, LocalId) -> Expr,
    },
    // TODO: 可変長
}

impl BuiltinConversionRule {
    pub fn ret_type(self) -> Type {
        match self {
            BuiltinConversionRule::Unary { ret, .. } => ret,
            BuiltinConversionRule::Binary { ret, .. } => ret,
            BuiltinConversionRule::Ternary { ret, .. } => ret,
        }
    }

    // TODO: 可変長引数が関わると返り値を変える必要あり
    pub fn arg_count(self) -> usize {
        self.arg_types().len()
    }

    pub fn arg_types(&self) -> &[Type] {
        match self {
            BuiltinConversionRule::Unary { args, .. } => args,
            BuiltinConversionRule::Binary { args, .. } => args,
            BuiltinConversionRule::Ternary { args, .. } => args,
        }
    }

    pub fn from_builtin(builtin: ast::Builtin) -> BuiltinConversionRule {
        use ast::Builtin;

        match builtin {
            Builtin::Display => BuiltinConversionRule::Unary {
                // TODO: 一旦Stringのみ
                args: [Type::Val(ValType::String)],
                ret: Type::Val(ValType::Nil),
                to_ir: Expr::Display,
            },
            Builtin::Add => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Int),
                to_ir: Expr::Add,
            },
            Builtin::Sub => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Int),
                to_ir: Expr::Sub,
            },
            Builtin::WriteChar => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Char)],
                ret: Type::Val(ValType::Nil),
                to_ir: Expr::WriteChar,
            },
            Builtin::IsPair => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsPair,
            },
            Builtin::IsSymbol => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsSymbol,
            },
            Builtin::IsString => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsString,
            },
            Builtin::IsNumber => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsNumber,
            },
            Builtin::IsBoolean => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsBoolean,
            },
            Builtin::IsProcedure => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsProcedure,
            },
            Builtin::IsChar => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsChar,
            },
            Builtin::IsVector => BuiltinConversionRule::Unary {
                args: [Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsVector,
            },
            Builtin::VectorLength => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Vector)],
                ret: Type::Val(ValType::Int),
                to_ir: Expr::VectorLength,
            },
            Builtin::VectorRef => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Vector), Type::Val(ValType::Int)],
                ret: Type::Boxed,
                to_ir: Expr::VectorRef,
            },
            Builtin::VectorSet => BuiltinConversionRule::Ternary {
                args: [
                    Type::Val(ValType::Vector),
                    Type::Val(ValType::Int),
                    Type::Boxed,
                ],
                ret: Type::Val(ValType::Nil),
                to_ir: Expr::VectorSet,
            },
            Builtin::Eq => BuiltinConversionRule::Binary {
                args: [Type::Boxed, Type::Boxed],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::Eq,
            },
            Builtin::Car => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Cons)],
                ret: Type::Boxed,
                to_ir: Expr::Car,
            },
            Builtin::Cdr => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Cons)],
                ret: Type::Boxed,
                to_ir: Expr::Cdr,
            },
            Builtin::SymbolToString => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Symbol)],
                ret: Type::Val(ValType::String),
                to_ir: Expr::SymbolToString,
            },
            Builtin::NumberToString => BuiltinConversionRule::Unary {
                // TODO: 一般のnumberに使えるように
                args: [Type::Val(ValType::Int)],
                ret: Type::Val(ValType::String),
                to_ir: Expr::NumberToString,
            },
            Builtin::EqNum => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::EqNum,
            },
            Builtin::Lt => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::Lt,
            },
            Builtin::Gt => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::Gt,
            },
            Builtin::Le => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::Le,
            },
            Builtin::Ge => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::Ge,
            },
        }
    }
}
