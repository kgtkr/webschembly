use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::*;
use crate::ir_generator::IrGenerator;
use crate::{
    ast::{self, Desugared, TailCall, Used},
    x::{RunX, TypeMap, type_map},
};
use typed_index_collections::TiVec;

#[derive(Debug, Clone)]
pub struct Config {
    pub allow_set_builtin: bool,
}

pub fn generate_module(
    ir_generator: &mut IrGenerator,
    ast: &ast::Ast<ast::Final>,
    config: Config,
) -> Module {
    let module_gen = ModuleGenerator::new(config, ir_generator, ast);

    module_gen.generate()
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

        let globals = self
            .ast
            .x
            .get_ref(type_map::key::<Used>())
            .global_vars
            .clone()
            .into_iter()
            .map(|id| self.global_id(id))
            .collect::<FxHashSet<_>>();
        let meta = Meta {
            local_metas: self.local_metas,
            global_metas: self.global_metas,
        };

        Module {
            globals,
            funcs: self.funcs.into_iter().map(|f| f.unwrap()).collect(),
            entry: func_id,
            meta,
        }
    }

    fn global_id(&mut self, id: ast::GlobalVarId) -> GlobalId {
        let global_id = self.ir_generator.global_id(id);
        let ast_meta = self
            .ast
            .x
            .get_ref(type_map::key::<Used>())
            .global_metas
            .get(&id);
        if let Some(ast_meta) = ast_meta {
            self.global_metas
                .entry(global_id)
                .or_insert_with(|| VarMeta {
                    name: ast_meta.name.clone(),
                });
        }
        global_id
    }

    fn gen_func(
        &mut self,
        x: &RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> FuncId {
        let id = self.funcs.push_and_get_key(None);
        let func = FuncGenerator::new(self, id).lambda_gen(x, lambda);
        self.funcs[id] = Some(func);

        id
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
            expr: Expr::InitModule,
        });

        self.define_all_ast_local_and_create_mut_cell(
            &self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .defines,
        );

        self.gen_exprs(Some(boxed_local), &self.module_generator.ast.exprs);
        self.close_bb(Some(BasicBlockNext::Return(boxed_local)));
        Func {
            id: self.id,
            args: 0,
            ret_type: LocalType::Type(Type::Boxed),
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
            jit_strategy: FuncJitStrategy::Never,
        }
    }

    fn lambda_gen(
        mut self,
        x: &RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> Func {
        let self_closure = self.local(Type::Val(ValType::Closure));
        let args = self.local(LocalType::Args);
        // TODO: 引数の数が合っているかのチェック
        for (arg_idx, arg) in x.get_ref(type_map::key::<Used>()).args.iter().enumerate() {
            if self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .box_vars
                .contains(arg)
            {
                let local = self.local(LocalType::Type(Type::Boxed));
                self.exprs.push(ExprAssign {
                    local: Some(local),
                    expr: Expr::ArgsRef(args, arg_idx),
                });

                let mut_cell = self.define_ast_local(*arg);
                self.exprs.push(ExprAssign {
                    local: Some(mut_cell),
                    expr: Expr::CreateMutCell(Type::Boxed),
                });
                self.exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::SetMutCell(Type::Boxed, mut_cell, local),
                });
            } else {
                let arg_id = self.define_ast_local(*arg);
                self.exprs.push(ExprAssign {
                    local: Some(arg_id),
                    expr: Expr::ArgsRef(args, arg_idx),
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

        self.define_all_ast_local_and_create_mut_cell(&x.get_ref(type_map::key::<Used>()).defines);

        let ret = self.local(Type::Boxed);
        self.gen_exprs(Some(ret), &lambda.body);
        self.close_bb(Some(BasicBlockNext::Return(ret)));
        Func {
            id: self.id,
            args: 2,
            ret_type: LocalType::Type(Type::Boxed),
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
            jit_strategy: FuncJitStrategy::Lambda { args },
        }
    }

    fn local<T: Into<LocalType>>(&mut self, typ: T) -> LocalId {
        self.locals.push_and_get_key(typ.into())
    }

    fn define_all_ast_local_and_create_mut_cell(&mut self, locals: &[ast::LocalVarId]) {
        for id in locals {
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
                let func_id = self.module_generator.gen_func(x, lambda);
                let func_local = self.local(ValType::FuncRef);
                let unboxed = self.local(Type::Val(ValType::Closure));
                self.exprs.push(ExprAssign {
                    local: Some(func_local),
                    expr: Expr::FuncRef(func_id),
                });
                self.exprs.push(ExprAssign {
                    local: Some(unboxed),
                    expr: Expr::Closure {
                        envs: captures,
                        func: func_local,
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

                    let args_local = self.local(LocalType::Args);
                    let mut args_locals = Vec::new();
                    for arg in args {
                        let arg_local = self.local(Type::Boxed);
                        self.gen_expr(Some(arg_local), arg);
                        args_locals.push(arg_local);
                    }
                    self.exprs.push(ExprAssign {
                        local: Some(args_local),
                        expr: Expr::Args(args_locals),
                    });

                    let is_tail = x.get_ref(type_map::key::<TailCall>()).is_tail;
                    let call_ref = ExprCallRef {
                        func: func_local,
                        args: vec![closure_local, args_local],
                        func_type: FuncType {
                            ret: LocalType::Type(Type::Boxed),
                            args: vec![
                                LocalType::Type(Type::Val(ValType::Closure)),
                                LocalType::Args,
                            ],
                        },
                    };
                    if is_tail {
                        self.close_bb(Some(BasicBlockNext::TailCallRef(call_ref)));
                    } else {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::CallRef(call_ref),
                        });
                    }
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
                    let global = self.module_generator.global_id(*id);
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
                            let global = self.module_generator.global_id(*id);
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
            Builtin::Mul => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Int),
                to_ir: Expr::Mul,
            },
            Builtin::Div => BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Int),
                to_ir: Expr::Div,
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

#[derive(Debug, Clone)]
struct BasicBlockOptionalNext {
    pub exprs: Vec<ExprAssign>,
    pub next: Option<BasicBlockNext>,
}
