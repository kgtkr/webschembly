use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir_generator::GlobalManager;
use crate::{VecMap, ir::*};
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
    global_manager: &mut GlobalManager,
    ast: &ast::Ast<ast::Final>,
    config: Config,
) -> Module {
    let module_gen = ModuleGenerator::new(config, global_manager, ast);

    module_gen.generate()
}

#[derive(Debug)]
struct ModuleGenerator<'a> {
    global_manager: &'a mut GlobalManager,
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
        ir_generator: &'a mut GlobalManager,
        ast: &'a ast::Ast<ast::Final>,
    ) -> Self {
        Self {
            ast,
            global_manager: ir_generator,
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
        let global_id = self.global_manager.global_id(id);
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
    locals: VecMap<LocalId, Local>,
    local_ids: FxHashMap<ast::LocalVarId, LocalId>,
    bbs: VecMap<BasicBlockId, BasicBlock>,
    module_generator: &'a mut ModuleGenerator<'b>,
    exprs: Vec<ExprAssign>,
    current_bb_id: Option<BasicBlockId>,
}

impl<'a, 'b> FuncGenerator<'a, 'b> {
    fn new(module_generator: &'a mut ModuleGenerator<'b>, id: FuncId) -> Self {
        Self {
            id,
            locals: VecMap::new(),
            local_ids: FxHashMap::default(),
            bbs: VecMap::new(),
            module_generator,
            exprs: Vec::new(),
            current_bb_id: None,
        }
    }

    fn entry_gen(mut self) -> Func {
        let obj_local = self.local(Type::Obj);

        self.exprs.push(ExprAssign {
            local: None,
            expr: Expr::InitModule,
        });

        self.define_all_ast_local_and_create_ref(
            &self
                .module_generator
                .ast
                .x
                .get_ref(type_map::key::<Used>())
                .defines,
        );

        let bb_entry = self.bbs.allocate_key();
        self.current_bb_id = Some(bb_entry);
        self.gen_exprs(Some(obj_local), &self.module_generator.ast.exprs);
        self.close_bb(BasicBlockNext::Terminator(BasicBlockTerminator::Return(
            obj_local,
        )));
        Func {
            id: self.id,
            args: vec![],
            ret_type: LocalType::Type(Type::Obj),
            locals: self.locals,
            bb_entry,
            bbs: self.bbs,
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
                let local = self.local(LocalType::Type(Type::Obj));
                self.exprs.push(ExprAssign {
                    local: Some(local),
                    expr: Expr::ArgsRef(args, arg_idx),
                });

                let ref_ = self.define_ast_local(*arg);
                self.exprs.push(ExprAssign {
                    local: Some(ref_),
                    expr: Expr::CreateRef(Type::Obj),
                });
                self.exprs.push(ExprAssign {
                    local: None,
                    expr: Expr::SetRef(Type::Obj, ref_, local),
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
            .map(|id| self.locals[*self.local_ids.get(id).unwrap()].typ)
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

        self.define_all_ast_local_and_create_ref(&x.get_ref(type_map::key::<Used>()).defines);

        let bb_entry = self.bbs.allocate_key();
        self.current_bb_id = Some(bb_entry);
        let ret = self.local(Type::Obj);
        self.gen_exprs(Some(ret), &lambda.body);
        self.close_bb(BasicBlockNext::Terminator(BasicBlockTerminator::Return(
            ret,
        )));
        Func {
            id: self.id,
            args: vec![self_closure, args],
            ret_type: LocalType::Type(Type::Obj),
            locals: self.locals,
            bb_entry,
            bbs: self.bbs,
        }
    }

    fn local<T: Into<LocalType>>(&mut self, typ: T) -> LocalId {
        self.locals.push_with(|id| Local {
            id,
            typ: typ.into(),
        })
    }

    fn define_all_ast_local_and_create_ref(&mut self, locals: &[ast::LocalVarId]) {
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
                    expr: Expr::CreateRef(Type::Obj),
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
                LocalType::Ref(Type::Obj)
            } else {
                LocalType::Type(Type::Obj)
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
                    let val_type_local = self.local(Type::Val(ValType::Bool));
                    self.exprs.push(ExprAssign {
                        local: Some(val_type_local),
                        expr: Expr::Bool(*b),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::ToObj(ValType::Bool, val_type_local),
                    });
                }
                ast::Const::Int(i) => {
                    let val_type_local = self.local(Type::Val(ValType::Int));
                    self.exprs.push(ExprAssign {
                        local: Some(val_type_local),
                        expr: Expr::Int(*i),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::ToObj(ValType::Int, val_type_local),
                    });
                }
                ast::Const::String(s) => {
                    let val_type_local = self.local(Type::Val(ValType::String));
                    self.exprs.push(ExprAssign {
                        local: Some(val_type_local),
                        expr: Expr::String(s.clone()),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::ToObj(ValType::String, val_type_local),
                    });
                }
                ast::Const::Nil => {
                    let val_type_local = self.local(Type::Val(ValType::Nil));
                    self.exprs.push(ExprAssign {
                        local: Some(val_type_local),
                        expr: Expr::Nil,
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::ToObj(ValType::Nil, val_type_local),
                    });
                }
                ast::Const::Char(c) => {
                    let val_type_local = self.local(Type::Val(ValType::Char));
                    self.exprs.push(ExprAssign {
                        local: Some(val_type_local),
                        expr: Expr::Char(*c),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::ToObj(ValType::Char, val_type_local),
                    });
                }
                ast::Const::Symbol(s) => {
                    let string = self.local(Type::Val(ValType::String));
                    let val_type_local = self.local(Type::Val(ValType::Symbol));
                    self.exprs.push(ExprAssign {
                        local: Some(string),
                        expr: Expr::String(s.clone()),
                    });
                    self.exprs.push(ExprAssign {
                        local: Some(val_type_local),
                        expr: Expr::StringToSymbol(string),
                    });
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::ToObj(ValType::Symbol, val_type_local),
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
                let val_type_local = self.local(Type::Val(ValType::Closure));
                self.exprs.push(ExprAssign {
                    local: Some(func_local),
                    expr: Expr::FuncRef(func_id),
                });
                self.exprs.push(ExprAssign {
                    local: Some(val_type_local),
                    expr: Expr::Closure {
                        envs: captures,
                        func: func_local,
                    },
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::ToObj(ValType::Closure, val_type_local),
                });
            }
            ast::Expr::If(_, ast::If { cond, then, els }) => {
                let obj_cond_local = self.local(Type::Obj);
                self.gen_expr(Some(obj_cond_local), cond);

                // TODO: condがboolかのチェック
                let cond_local = self.local(Type::Val(ValType::Bool));
                self.exprs.push(ExprAssign {
                    local: Some(cond_local),
                    expr: Expr::FromObj(ValType::Bool, obj_cond_local),
                });

                let then_bb_id = self.bbs.allocate_key();
                let else_bb_id = self.bbs.allocate_key();
                let merge_bb_id = self.bbs.allocate_key();
                self.close_bb(BasicBlockNext::If(cond_local, then_bb_id, else_bb_id));

                self.current_bb_id = Some(then_bb_id);
                let then_result = self.local(Type::Obj);
                self.gen_expr(Some(then_result), then);
                self.close_bb(BasicBlockNext::Jump(merge_bb_id));

                self.current_bb_id = Some(else_bb_id);
                let els_result = self.local(Type::Obj);
                self.gen_expr(Some(els_result), els);
                self.close_bb(BasicBlockNext::Jump(merge_bb_id));

                self.current_bb_id = Some(merge_bb_id);
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::Phi(vec![
                        PhiIncomingValue {
                            bb: then_bb_id,
                            local: then_result,
                        },
                        PhiIncomingValue {
                            bb: else_bb_id,
                            local: els_result,
                        },
                    ]),
                });
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
                            let obj_arg_local = self.local(Type::Obj);
                            self.gen_expr(Some(obj_arg_local), arg);
                            let arg_local = match typ {
                                Type::Obj => obj_arg_local,
                                Type::Val(val_type) => {
                                    let val_type_arg_local = self.local(Type::Val(*val_type));
                                    // TODO: 動的型チェック
                                    self.exprs.push(ExprAssign {
                                        local: Some(val_type_arg_local),
                                        expr: Expr::FromObj(*val_type, obj_arg_local),
                                    });
                                    val_type_arg_local
                                }
                            };
                            arg_locals.push(arg_local);
                        }

                        let ret_local = match rule.ret_type() {
                            Type::Obj => self.local(Type::Obj),
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
                            Type::Obj => {
                                self.exprs.push(ExprAssign {
                                    local: result,
                                    expr: Expr::Move(ret_local),
                                });
                            }
                            Type::Val(val_type) => {
                                self.exprs.push(ExprAssign {
                                    local: result,
                                    expr: Expr::ToObj(val_type, ret_local),
                                });
                            }
                        }
                    }
                } else {
                    let obj_func_local = self.local(Type::Obj);
                    self.gen_expr(Some(obj_func_local), func);

                    // TODO: funcがクロージャかのチェック
                    let closure_local = self.local(ValType::Closure);
                    let func_local = self.local(ValType::FuncRef);
                    self.exprs.push(ExprAssign {
                        local: Some(closure_local),
                        expr: Expr::FromObj(ValType::Closure, obj_func_local),
                    });
                    self.exprs.push(ExprAssign {
                        local: Some(func_local),
                        expr: Expr::ClosureFuncRef(closure_local),
                    });

                    let args_local = self.local(LocalType::Args);
                    let mut args_locals = Vec::new();
                    for arg in args {
                        let arg_local = self.local(Type::Obj);
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
                            ret: LocalType::Type(Type::Obj),
                            args: vec![
                                LocalType::Type(Type::Val(ValType::Closure)),
                                LocalType::Args,
                            ],
                        },
                    };
                    if is_tail {
                        self.close_bb(BasicBlockNext::Terminator(
                            BasicBlockTerminator::TailCallRef(call_ref),
                        ));
                        self.current_bb_id = Some(self.bbs.allocate_key());
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
                            expr: Expr::DerefRef(Type::Obj, *self.local_ids.get(id).unwrap()),
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
                            let obj_local = self.local(Type::Obj);
                            self.gen_expr(Some(obj_local), expr);
                            let local = self.local_ids.get(id).unwrap();
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::SetRef(Type::Obj, *local, obj_local),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(obj_local),
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
                            let local = self.local(Type::Obj);
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
                    let obj_local = self.local(Type::Obj);
                    self.gen_expr(Some(obj_local), sexpr);
                    vec_locals.push(obj_local);
                }
                let val_type_local = self.local(Type::Val(ValType::Vector));
                self.exprs.push(ExprAssign {
                    local: Some(val_type_local),
                    expr: Expr::Vector(vec_locals),
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::ToObj(ValType::Vector, val_type_local),
                });
            }
            ast::Expr::Cons(_, cons) => {
                let car_local = self.local(Type::Obj);
                self.gen_expr(Some(car_local), &cons.car);
                let cdr_local = self.local(Type::Obj);
                self.gen_expr(Some(cdr_local), &cons.cdr);

                let val_type_local = self.local(Type::Val(ValType::Cons));
                self.exprs.push(ExprAssign {
                    local: Some(val_type_local),
                    expr: Expr::Cons(car_local, cdr_local),
                });
                self.exprs.push(ExprAssign {
                    local: result,
                    expr: Expr::ToObj(ValType::Cons, val_type_local),
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
            let val_type_local = self.local(Type::Val(ValType::Nil));
            self.exprs.push(ExprAssign {
                local: Some(val_type_local),
                expr: Expr::Nil,
            });
            self.exprs.push(ExprAssign {
                local: result,
                expr: Expr::ToObj(ValType::Nil, val_type_local),
            });
        }
    }

    fn close_bb(&mut self, next: BasicBlockNext) {
        let bb_exprs = std::mem::take(&mut self.exprs);
        self.bbs.insert(self.current_bb_id.unwrap(), BasicBlock {
            id: self.current_bb_id.unwrap(),
            exprs: bb_exprs,
            next,
        });
        self.current_bb_id = None;
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
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsPair,
            },
            Builtin::IsSymbol => BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsSymbol,
            },
            Builtin::IsString => BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsString,
            },
            Builtin::IsNumber => BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsNumber,
            },
            Builtin::IsBoolean => BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsBoolean,
            },
            Builtin::IsProcedure => BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsProcedure,
            },
            Builtin::IsChar => BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::IsChar,
            },
            Builtin::IsVector => BuiltinConversionRule::Unary {
                args: [Type::Obj],
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
                ret: Type::Obj,
                to_ir: Expr::VectorRef,
            },
            Builtin::VectorSet => BuiltinConversionRule::Ternary {
                args: [
                    Type::Val(ValType::Vector),
                    Type::Val(ValType::Int),
                    Type::Obj,
                ],
                ret: Type::Val(ValType::Nil),
                to_ir: Expr::VectorSet,
            },
            Builtin::Eq => BuiltinConversionRule::Binary {
                args: [Type::Obj, Type::Obj],
                ret: Type::Val(ValType::Bool),
                to_ir: Expr::Eq,
            },
            Builtin::Cons => BuiltinConversionRule::Binary {
                args: [Type::Obj, Type::Obj],
                ret: Type::Val(ValType::Cons),
                to_ir: Expr::Cons,
            },
            Builtin::Car => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Cons)],
                ret: Type::Obj,
                to_ir: Expr::Car,
            },
            Builtin::Cdr => BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Cons)],
                ret: Type::Obj,
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
