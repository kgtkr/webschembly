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
struct IrGenerator {
    funcs: TiVec<FuncId, Func>,
    box_vars: FxHashSet<ast::LocalVarId>,
    config: Config,
    // メタ情報
    local_metas: FxHashMap<(FuncId, LocalId), VarMeta>,
    global_metas: FxHashMap<GlobalId, VarMeta>,
    ast_local_metas: FxHashMap<ast::LocalVarId, ast::VarMeta>,
    ast_global_metas: FxHashMap<ast::GlobalVarId, ast::VarMeta>,
}

impl IrGenerator {
    fn new(
        config: Config,
        ast_local_metas: FxHashMap<ast::LocalVarId, ast::VarMeta>,
        ast_global_metas: FxHashMap<ast::GlobalVarId, ast::VarMeta>,
    ) -> Self {
        Self {
            funcs: TiVec::new(),
            box_vars: FxHashSet::default(),
            config,
            local_metas: FxHashMap::default(),
            global_metas: FxHashMap::default(),
            ast_local_metas,
            ast_global_metas,
        }
    }

    fn generate(mut self, ast: &ast::Ast<ast::Final>) -> (Ir, Meta) {
        let func_id = self.funcs.next_key();
        self.box_vars = ast.x.get_ref(type_map::key::<Used>()).box_vars.clone();
        let func = FuncGenerator::new(&mut self, func_id).entry_gen(ast);
        self.funcs.push(func);

        let meta = self.meta();
        (
            Ir {
                funcs: self.funcs,
                entry: func_id,
            },
            meta,
        )
    }

    fn gen_func(
        &mut self,
        x: RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> (FuncId, FuncId) {
        let id = self.funcs.next_key();
        let func = FuncGenerator::new(self, id).lambda_gen(x, lambda);
        self.funcs.push(func);

        let boxed_id = self.funcs.next_key();
        let boxed_func = FuncGenerator::new(self, boxed_id).boxed_func_gen(id, lambda.args.len());
        self.funcs.push(boxed_func);

        (id, boxed_id)
    }

    fn meta(&self) -> Meta {
        Meta {
            local_metas: self.local_metas.clone(),
            global_metas: self.global_metas.clone(),
        }
    }
}

#[derive(Debug)]
struct FuncGenerator<'a> {
    id: FuncId,
    locals: TiVec<LocalId, LocalType>,
    local_ids: FxHashMap<ast::LocalVarId, LocalId>,
    bbs: TiVec<BasicBlockId, BasicBlockOptionalNext>,
    next_undecided_bb_ids: FxHashSet<BasicBlockId>,
    ir_generator: &'a mut IrGenerator,
    exprs: Vec<ExprAssign>,
}

impl<'a> FuncGenerator<'a> {
    fn new(ir_generator: &'a mut IrGenerator, id: FuncId) -> Self {
        Self {
            id,
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
        self.gen_exprs(Some(boxed_local), &ast.exprs);
        self.close_bb(Some(BasicBlockNext::Return));
        Func {
            id: self.id,
            args: 0,
            rets: vec![boxed_local],
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
        x: RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> Func {
        let self_closure = self.local(Type::Val(ValType::Closure));
        for arg in &x.get_ref(type_map::key::<Used>()).args {
            self.define_ast_local(*arg);
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
                expr: Expr::ClosureEnv(env_types.clone(), self_closure, i),
            });
        }

        for id in &x.get_ref(type_map::key::<Used>()).defines {
            let local = self.define_ast_local(*id);
            if self.ir_generator.box_vars.contains(id) {
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
            rets: vec![ret],
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
        let ast_meta = self.ir_generator.ast_local_metas.get(&id).cloned();
        let local = self.local(if self.ir_generator.box_vars.contains(&id) {
            LocalType::MutCell(Type::Boxed)
        } else {
            LocalType::Type(Type::Boxed)
        });
        self.local_ids.insert(id, local);
        if let Some(ast_meta) = ast_meta {
            self.ir_generator
                .local_metas
                .insert((self.id, local), VarMeta {
                    name: ast_meta.name,
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
                let (func_id, boxed_func_id) = self.ir_generator.gen_func(x.clone(), lambda);
                let func_local = self.local(Type::Val(ValType::FuncRef));
                let boxed_func_local = self.local(Type::Val(ValType::FuncRef));
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
                    let builtin_typ = builtin_func_type(builtin);
                    debug_assert!(builtin_typ.rets.len() == 1);
                    let ret_type = *builtin_typ.rets.first().unwrap();
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

                        let ret_local = match ret_type {
                            Type::Boxed => self.local(Type::Boxed),
                            Type::Val(val_type) => self.local(Type::Val(val_type)),
                        };
                        // TODO: きれいに書き直す
                        let expr = match builtin {
                            ast::Builtin::Display => Expr::Display(arg_locals[0]),
                            ast::Builtin::Add => Expr::Add(arg_locals[0], arg_locals[1]),
                            ast::Builtin::Sub => Expr::Sub(arg_locals[0], arg_locals[1]),
                            ast::Builtin::WriteChar => Expr::WriteChar(arg_locals[0]),
                            ast::Builtin::IsPair => Expr::IsPair(arg_locals[0]),
                            ast::Builtin::IsSymbol => Expr::IsSymbol(arg_locals[0]),
                            ast::Builtin::IsString => Expr::IsString(arg_locals[0]),
                            ast::Builtin::IsNumber => Expr::IsNumber(arg_locals[0]),
                            ast::Builtin::IsBoolean => Expr::IsBoolean(arg_locals[0]),
                            ast::Builtin::IsProcedure => Expr::IsProcedure(arg_locals[0]),
                            ast::Builtin::IsChar => Expr::IsChar(arg_locals[0]),
                            ast::Builtin::IsVector => Expr::IsVector(arg_locals[0]),
                            ast::Builtin::VectorLength => Expr::VectorLength(arg_locals[0]),
                            ast::Builtin::VectorRef => {
                                Expr::VectorRef(arg_locals[0], arg_locals[1])
                            }
                            ast::Builtin::VectorSet => {
                                Expr::VectorSet(arg_locals[0], arg_locals[1], arg_locals[2])
                            }
                            ast::Builtin::Eq => Expr::Eq(arg_locals[0], arg_locals[1]),
                            ast::Builtin::Car => Expr::Car(arg_locals[0]),
                            ast::Builtin::Cdr => Expr::Cdr(arg_locals[0]),
                            ast::Builtin::SymbolToString => Expr::SymbolToString(arg_locals[0]),
                            ast::Builtin::NumberToString => Expr::NumberToString(arg_locals[0]),
                            ast::Builtin::EqNum => Expr::EqNum(arg_locals[0], arg_locals[1]),
                            ast::Builtin::Lt => Expr::Lt(arg_locals[0], arg_locals[1]),
                            ast::Builtin::Gt => Expr::Gt(arg_locals[0], arg_locals[1]),
                            ast::Builtin::Le => Expr::Le(arg_locals[0], arg_locals[1]),
                            ast::Builtin::Ge => Expr::Ge(arg_locals[0], arg_locals[1]),
                        };
                        self.exprs.push(ExprAssign {
                            local: Some(ret_local),
                            expr,
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
                    self.gen_expr(Some(boxed_func_local), func);

                    // TODO: funcがクロージャかのチェック
                    let closure_local = self.local(Type::Val(ValType::Closure));
                    let func_local = self.local(Type::Val(ValType::FuncRef));
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
                    arg_locals.push(closure_local); // 第一引数にクロージャを渡す
                    for arg in args {
                        let arg_local = self.local(Type::Boxed);
                        self.gen_expr(Some(arg_local), arg);
                        arg_locals.push(arg_local);
                    }
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::CallRef(
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
                        if self.ir_generator.box_vars.contains(id) {
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
        let ast_meta = self.ir_generator.ast_global_metas.get(&id);
        let id = GlobalId::from(id.0);
        if let Some(ast_meta) = ast_meta {
            self.ir_generator.global_metas.insert(id, VarMeta {
                name: ast_meta.name.clone(),
            });
        }
        id
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

pub fn generate_ir(
    ast: &ast::Ast<ast::Final>,
    config: Config,
    ast_local_metas: FxHashMap<ast::LocalVarId, ast::VarMeta>,
    ast_global_metas: FxHashMap<ast::GlobalVarId, ast::VarMeta>,
) -> (Ir, Meta) {
    let ir_gen = IrGenerator::new(config, ast_local_metas, ast_global_metas);

    ir_gen.generate(ast)
}

// TODO: ここに置くか微妙。stdlibのためにpubにしている
pub fn builtin_func_type(builtin: ast::Builtin) -> FuncType {
    use ast::Builtin;
    match builtin {
        Builtin::Display => FuncType {
            args: ti_vec![Type::Val(ValType::String)], // TODO: 一旦Stringのみ
            rets: ti_vec![Type::Val(ValType::Nil)],
        },
        Builtin::Add => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Int)],
        },
        Builtin::Sub => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Int)],
        },
        Builtin::WriteChar => FuncType {
            args: ti_vec![Type::Val(ValType::Char)],
            rets: ti_vec![Type::Val(ValType::Nil)],
        },
        Builtin::IsPair => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsSymbol => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsString => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsNumber => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsBoolean => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsProcedure => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsChar => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::IsVector => FuncType {
            args: ti_vec![Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::VectorLength => FuncType {
            args: ti_vec![Type::Val(ValType::Vector)],
            rets: ti_vec![Type::Val(ValType::Int)],
        },
        Builtin::VectorRef => FuncType {
            args: ti_vec![Type::Val(ValType::Vector), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Boxed],
        },
        Builtin::VectorSet => FuncType {
            args: ti_vec![
                Type::Val(ValType::Vector),
                Type::Val(ValType::Int),
                Type::Boxed
            ],
            rets: ti_vec![Type::Val(ValType::Nil)],
        },
        Builtin::Eq => FuncType {
            args: ti_vec![Type::Boxed, Type::Boxed],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::Car => FuncType {
            args: ti_vec![Type::Val(ValType::Cons)],
            rets: ti_vec![Type::Boxed],
        },
        Builtin::Cdr => FuncType {
            args: ti_vec![Type::Val(ValType::Cons)],
            rets: ti_vec![Type::Boxed],
        },
        Builtin::SymbolToString => FuncType {
            args: ti_vec![Type::Val(ValType::Symbol)],
            rets: ti_vec![Type::Val(ValType::String)],
        },
        Builtin::NumberToString => FuncType {
            args: ti_vec![Type::Val(ValType::Int)], // TODO: 一般のnumberに使えるように
            rets: ti_vec![Type::Val(ValType::String)],
        },
        Builtin::EqNum => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::Lt => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::Gt => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::Le => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
        Builtin::Ge => FuncType {
            args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: ti_vec![Type::Val(ValType::Bool)],
        },
    }
}
