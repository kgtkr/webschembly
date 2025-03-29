use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::*;
use crate::{
    ast::{self, Desugared, TailCall, Used},
    sexpr,
    x::{RunX, TypeMap, type_map},
};
use typed_index_collections::TiVec;

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
struct IrGenerator {
    funcs: Vec<Func>,
    box_vars: FxHashSet<ast::LocalVarId>,
    config: Config,
}

impl IrGenerator {
    fn new(config: Config) -> Self {
        Self {
            funcs: Vec::new(),
            box_vars: FxHashSet::default(),
            config,
        }
    }

    fn generate(mut self, ast: &ast::Ast<ast::Final>) -> Ir {
        self.box_vars = ast.x.get_ref(type_map::key::<Used>()).box_vars.clone();
        let func = FuncGenerator::new(&mut self).entry_gen(ast);
        let func_id = self.funcs.len();
        self.funcs.push(func);

        Ir {
            funcs: self.funcs,
            entry: func_id,
        }
    }

    fn gen_func(
        &mut self,
        x: RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> usize {
        let func = FuncGenerator::new(self).lambda_gen(x, lambda);
        let func_id = self.funcs.len();
        self.funcs.push(func);
        func_id
    }
}

#[derive(Debug)]
struct FuncGenerator<'a> {
    locals: TiVec<LocalId, LocalType>,
    local_ids: FxHashMap<ast::LocalVarId, LocalId>,
    bbs: TiVec<BasicBlockId, BasicBlockOptionalNext>,
    next_undecided_bb_ids: FxHashSet<BasicBlockId>,
    ir_generator: &'a mut IrGenerator,
    exprs: Vec<ExprAssign>,
}

impl<'a> FuncGenerator<'a> {
    fn new(ir_generator: &'a mut IrGenerator) -> Self {
        Self {
            locals: TiVec::new(),
            local_ids: FxHashMap::default(),
            bbs: TiVec::new(),
            next_undecided_bb_ids: FxHashSet::default(),
            ir_generator,
            exprs: Vec::new(),
        }
    }

    fn entry_gen(mut self, ast: &ast::Ast<ast::Final>) -> Func {
        let boxed_local = self.local(Type::Boxed);

        self.exprs.push(ExprAssign {
            local: None,
            expr: Expr::InitGlobals(
                ast.x
                    .get_ref(type_map::key::<Used>())
                    .global_vars
                    .iter()
                    .map(|x| x.0)
                    .max()
                    .map(|n| n + 1)
                    .unwrap_or(0),
            ),
        });
        self.gen_stats(Some(boxed_local), &ast.exprs);
        self.close_bb(Some(BasicBlockNext::Return));
        Func {
            args: 0,
            rets: vec![boxed_local],
            locals: self.locals,
            bb_entry: BasicBlockId::from(0), // TODO: もっと綺麗な書き方があるはず
            bbs: self
                .bbs
                .into_iter_enumerated()
                .into_iter()
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
        x: RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> Func {
        let self_closure = self.local(Type::Val(ValType::Closure));
        for arg in &x.get_ref(type_map::key::<Used>()).args {
            self.define_ast_local(*arg);
        }

        let mut restore_envs = Vec::new();
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
            restore_envs.push(ExprAssign {
                local: Some(env_local),
                expr: Expr::ClosureEnv(env_types.clone(), self_closure, i),
            });
        }

        let mut create_mut_cells = Vec::new();

        for id in &x.get_ref(type_map::key::<Used>()).defines {
            let local = self.define_ast_local(*id);
            if self.ir_generator.box_vars.contains(id) {
                create_mut_cells.push(ExprAssign {
                    local: Some(local),
                    expr: Expr::CreateMutCell,
                });
            }
        }

        let ret = self.local(Type::Boxed);

        self.exprs.extend(restore_envs);
        self.exprs.extend(create_mut_cells);
        self.gen_stats(Some(ret), &lambda.body);
        self.close_bb(Some(BasicBlockNext::Return));
        Func {
            args: lambda.args.len() + 1,
            rets: vec![ret],
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

    fn local<T: Into<LocalType>>(&mut self, typ: T) -> LocalId {
        self.locals.push_and_get_key(typ.into())
    }

    fn define_ast_local(&mut self, id: ast::LocalVarId) -> LocalId {
        let local = self.local(if self.ir_generator.box_vars.contains(&id) {
            LocalType::MutCell
        } else {
            LocalType::Type(Type::Boxed)
        });
        self.local_ids.insert(id, local);
        local
    }

    fn gen_stat(&mut self, result: Option<LocalId>, ast: &ast::Expr<ast::Final>) {
        match ast {
            ast::Expr::Literal(_, lit) => match lit {
                ast::Literal::Bool(b) => {
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
                ast::Literal::Int(i) => {
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
                ast::Literal::String(s) => {
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
                ast::Literal::Nil => {
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
                ast::Literal::Char(c) => {
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
                ast::Literal::Quote(sexpr) => {
                    self.quote(result, sexpr);
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
                let func_id: usize = self.ir_generator.gen_func(x.clone(), lambda);
                let unboxed = self.local(Type::Val(ValType::Closure));
                self.exprs.push(ExprAssign {
                    local: Some(unboxed),
                    expr: Expr::Closure(captures, func_id),
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::Box(ValType::Closure, unboxed),
                });
            }
            ast::Expr::If(_, ast::If { cond, then, els }) => {
                let boxed_cond_local = self.local(Type::Boxed);
                self.gen_stat(Some(boxed_cond_local), cond);

                // TODO: condがboolかのチェック
                let cond_local = self.local(Type::Val(ValType::Bool));
                self.exprs.push(ExprAssign {
                    local: Some(cond_local),
                    expr: Expr::Unbox(ValType::Bool, boxed_cond_local),
                });

                let bb_id = self.close_bb(None);

                let then_first_bb_id = self.bbs.next_key();
                self.gen_stat(result, then);
                let then_last_bb_id = self.close_bb(None);

                let else_first_bb_id = self.bbs.next_key();
                self.gen_stat(result, els);
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
                    && let Some(builtin) = Builtin::from_name(name)
                {
                    let builtin_typ = builtin.func_type();
                    debug_assert!(builtin_typ.rets.len() == 1);
                    let ret_type = builtin_typ.rets[0];
                    if builtin_typ.args.len() != args.len() {
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
                        for (typ, arg) in builtin_typ.args.iter().zip(args) {
                            let boxed_arg_local = self.local(Type::Boxed);
                            self.gen_stat(Some(boxed_arg_local), arg);
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

                        let ret_local = match ret_type {
                            Type::Boxed => self.local(Type::Boxed),
                            Type::Val(val_type) => self.local(Type::Val(val_type)),
                        };
                        self.exprs.push(ExprAssign {
                            local: Some(ret_local),
                            expr: Expr::Builtin(builtin, arg_locals),
                        });
                        match ret_type {
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
                    self.gen_stat(Some(boxed_func_local), func);

                    // TODO: funcがクロージャかのチェック
                    let func_local = self.local(Type::Val(ValType::Closure));
                    self.exprs.push(ExprAssign {
                        local: Some(func_local),
                        expr: Expr::Unbox(ValType::Closure, boxed_func_local),
                    });

                    // TODO: 引数の数が合っているかのチェック
                    let mut arg_locals = Vec::new();
                    arg_locals.push(func_local); // 第一引数にクロージャを渡す
                    for arg in args {
                        let arg_local = self.local(Type::Boxed);
                        self.gen_stat(Some(arg_local), arg);
                        arg_locals.push(arg_local);
                    }
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::CallClosure(
                            x.get_ref(type_map::key::<TailCall>()).is_tail,
                            func_local,
                            arg_locals,
                        ),
                    });
                }
            }
            ast::Expr::Var(x, _) => match &x.get_ref(type_map::key::<Used>()).var_id {
                ast::VarId::Local(id) => {
                    if self.ir_generator.box_vars.contains(id) {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::DerefMutCell(*self.local_ids.get(id).unwrap()),
                        });
                    } else {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::Move(*self.local_ids.get(id).unwrap()),
                        });
                    }
                }
                ast::VarId::Global(id) => {
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::GlobalGet(id.0),
                    });
                }
            },
            ast::Expr::Begin(_, ast::Begin { exprs: stats }) => {
                self.gen_stats(result, stats);
            }
            ast::Expr::Set(x, ast::Set { name, expr, .. }) => {
                match &x.get_ref(type_map::key::<Used>()).var_id {
                    ast::VarId::Local(id) => {
                        if self.ir_generator.box_vars.contains(id) {
                            let boxed_local = self.local(Type::Boxed);
                            self.gen_stat(Some(boxed_local), expr);
                            let local = self.local_ids.get(id).unwrap();
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::SetMutCell(*local, boxed_local),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(boxed_local),
                            });
                        } else {
                            let local = *self.local_ids.get(id).unwrap();
                            self.gen_stat(Some(local), expr);
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(local),
                            });
                        }
                    }
                    ast::VarId::Global(id) => {
                        if let Some(_) = Builtin::from_name(&name)
                            && !self.ir_generator.config.allow_set_builtin
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
                            self.gen_stat(Some(local), expr);
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::GlobalSet(id.0, local),
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
        }
    }

    fn quote(&mut self, result: Option<LocalId>, sexpr: &sexpr::SExpr) {
        match &sexpr.kind {
            sexpr::SExprKind::Bool(b) => {
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
            sexpr::SExprKind::Int(i) => {
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
            sexpr::SExprKind::String(s) => {
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
            sexpr::SExprKind::Symbol(s) => {
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
            sexpr::SExprKind::Nil => {
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
            sexpr::SExprKind::Char(c) => {
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
            sexpr::SExprKind::Cons(cons) => {
                let car_local = self.local(Type::Boxed);
                self.quote(Some(car_local), &cons.car);
                let cdr_local = self.local(Type::Boxed);
                self.quote(Some(cdr_local), &cons.cdr);

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
        }
    }

    fn gen_stats(&mut self, result: Option<LocalId>, stats: &[ast::Expr<ast::Final>]) {
        if let Some((last, rest)) = stats.split_last() {
            for stat in rest {
                self.gen_stat(None, stat);
            }
            self.gen_stat(result, last);
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

pub fn generate_ir(ast: &ast::Ast<ast::Final>, config: Config) -> Ir {
    let ir_gen = IrGenerator::new(config);

    ir_gen.generate(ast)
}
