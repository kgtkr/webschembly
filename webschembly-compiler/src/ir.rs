use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    ast::{self, Desugared, TailCall, Used},
    sexpr,
    x::{RunX, TypeMap, type_map},
};
use derive_more::{From, Into};
use strum::IntoEnumIterator;
use typed_index_collections::TiVec;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum LocalType {
    MutCell, // 中身はBoxed固定
    Type(Type),
}

impl From<Type> for LocalType {
    fn from(typ: Type) -> Self {
        Self::Type(typ)
    }
}

impl From<ValType> for LocalType {
    fn from(typ: ValType) -> Self {
        Self::Type(Type::from(typ))
    }
}

impl LocalType {
    pub fn to_type(&self) -> Type {
        match self {
            LocalType::MutCell => Type::Boxed,
            LocalType::Type(typ) => *typ,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Type {
    Boxed,
    Val(ValType),
}

impl From<ValType> for Type {
    fn from(typ: ValType) -> Self {
        Self::Val(typ)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum ValType {
    Bool,
    Int,
    String,
    Symbol,
    Nil,
    Cons,
    Closure,
    Char,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    String(String),
    StringToSymbol(usize),
    Nil,
    Char(char),
    Cons(usize, usize),
    CreateMutCell,
    DerefMutCell(usize),
    SetMutCell(usize /* mutcell */, usize /* value */),
    Closure(Vec<usize>, usize),
    CallClosure(bool, usize, Vec<usize>),
    Move(usize),
    Box(ValType, usize),
    Unbox(ValType, usize),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        usize,          /* closure */
        usize,          /* env index */
    ),
    GlobalSet(usize, usize),
    GlobalGet(usize),
    // Builtin = BuiltinClosure + CallClosureだが後から最適化するのは大変なので一旦分けておく
    Builtin(ast::Builtin, Vec<usize>),
    GetBuiltin(ast::Builtin),
    SetBuiltin(ast::Builtin, usize),
    Error(usize),
    InitGlobals(usize),  // global count
    InitBuiltins(usize), // builtin count
}

#[derive(Debug, Clone)]
pub struct ExprAssign {
    pub local: Option<usize>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
struct BasicBlockOptionalNext {
    pub exprs: Vec<ExprAssign>,
    pub next: Option<BasicBlockNext>,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub exprs: Vec<ExprAssign>,
    pub next: BasicBlockNext,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct BasicBlockId(usize);

// 閉路を作ってはいけない
#[derive(Debug, Clone, Copy)]
pub enum BasicBlockNext {
    If(usize, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Return,
}

#[derive(Debug, Clone)]
pub struct Func {
    pub locals: Vec<LocalType>,
    // localsの先頭何個が引数か
    pub args: usize,
    // localsのうちどれが返り値か
    pub rets: Vec<usize>,
    pub bb_entry: BasicBlockId,
    pub bbs: TiVec<BasicBlockId, BasicBlock>,
}

impl Func {
    pub fn arg_types(&self) -> Vec<Type> {
        (0..self.args).map(|i| self.locals[i].to_type()).collect()
    }

    pub fn ret_types(&self) -> Vec<Type> {
        self.rets
            .iter()
            .map(|&ret| self.locals[ret].to_type())
            .collect()
    }

    pub fn func_type(&self) -> FuncType {
        FuncType {
            args: self.arg_types(),
            rets: self.ret_types(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub args: Vec<Type>,
    pub rets: Vec<Type>,
}

#[derive(Debug, Clone)]
pub struct Ir {
    pub funcs: Vec<Func>,
    pub entry: usize,
}

impl Ir {
    pub fn from_ast(ast: &ast::Ast<ast::Final>, config: Config) -> Ir {
        let ir_gen = IrGenerator::new(config);

        ir_gen.generate(ast)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub allow_set_builtin: bool,
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
    locals: Vec<LocalType>,
    local_ids: FxHashMap<ast::LocalVarId, usize>,
    bbs: TiVec<BasicBlockId, BasicBlockOptionalNext>,
    next_undecided_bb_ids: FxHashSet<BasicBlockId>,
    ir_generator: &'a mut IrGenerator,
}

impl<'a> FuncGenerator<'a> {
    fn new(ir_generator: &'a mut IrGenerator) -> Self {
        Self {
            locals: Vec::new(),
            local_ids: FxHashMap::default(),
            bbs: TiVec::new(),
            next_undecided_bb_ids: FxHashSet::default(),
            ir_generator,
        }
    }

    fn entry_gen(mut self, ast: &ast::Ast<ast::Final>) -> Func {
        let boxed_local = self.local(Type::Boxed);

        let mut block_gen = BlockGenerator::new(&mut self);
        block_gen.exprs.push(ExprAssign {
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
        block_gen.exprs.push(ExprAssign {
            local: None,
            expr: Expr::InitBuiltins(ast::Builtin::iter().len()),
        });
        block_gen.gen_stats(Some(boxed_local), &ast.exprs);
        block_gen.close_bb(Some(BasicBlockNext::Return));
        Func {
            args: 0,
            rets: vec![boxed_local],
            locals: self.locals,
            bb_entry: BasicBlockId::from(0), // TODO: もっと綺麗な書き方があるはず
            bbs: self
                .bbs
                .into_iter()
                .map(|bb| BasicBlock {
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

        let mut block_gen = BlockGenerator::new(&mut self);
        block_gen.exprs.extend(restore_envs);
        block_gen.exprs.extend(create_mut_cells);
        block_gen.gen_stats(Some(ret), &lambda.body);
        block_gen.close_bb(Some(BasicBlockNext::Return));
        Func {
            args: lambda.args.len() + 1,
            rets: vec![ret],
            locals: self.locals,
            bb_entry: BasicBlockId::from(0), // TODO: もっと綺麗な書き方があるはず
            bbs: self
                .bbs
                .into_iter()
                .map(|bb| BasicBlock {
                    exprs: bb.exprs,
                    next: bb.next.unwrap(),
                })
                .collect(),
        }
    }

    fn local<T: Into<LocalType>>(&mut self, typ: T) -> usize {
        let local = self.locals.len();
        self.locals.push(typ.into());
        local
    }

    fn define_ast_local(&mut self, id: ast::LocalVarId) -> usize {
        let local = self.local(if self.ir_generator.box_vars.contains(&id) {
            LocalType::MutCell
        } else {
            LocalType::Type(Type::Boxed)
        });
        self.local_ids.insert(id, local);
        local
    }
}

#[derive(Debug)]
struct BlockGenerator<'a, 'b> {
    exprs: Vec<ExprAssign>,
    func_gen: &'b mut FuncGenerator<'a>,
}

impl<'a, 'b> BlockGenerator<'a, 'b> {
    fn new(func_gen: &'b mut FuncGenerator<'a>) -> Self {
        Self {
            exprs: Vec::new(),
            func_gen,
        }
    }

    fn gen_stat(&mut self, result: Option<usize>, ast: &ast::Expr<ast::Final>) {
        match ast {
            ast::Expr::Literal(_, lit) => match lit {
                ast::Literal::Bool(b) => {
                    let unboxed = self.func_gen.local(Type::Val(ValType::Bool));
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
                    let unboxed = self.func_gen.local(Type::Val(ValType::Int));
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
                    let unboxed = self.func_gen.local(Type::Val(ValType::String));
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
                    let unboxed = self.func_gen.local(Type::Val(ValType::Nil));
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
                    let unboxed = self.func_gen.local(Type::Val(ValType::Char));
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
                    .map(|id| *self.func_gen.local_ids.get(id).unwrap())
                    .collect::<Vec<_>>();
                let func_id: usize = self.func_gen.ir_generator.gen_func(x.clone(), lambda);
                let unboxed = self.func_gen.local(Type::Val(ValType::Closure));
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
                let boxed_cond_local = self.func_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_cond_local), cond);

                // TODO: condがboolかのチェック
                let cond_local = self.func_gen.local(Type::Val(ValType::Bool));
                self.exprs.push(ExprAssign {
                    local: Some(cond_local),
                    expr: Expr::Unbox(ValType::Bool, boxed_cond_local),
                });

                let bb_id = self.close_bb(None);

                let then_first_bb_id = self.func_gen.bbs.next_key();

                let then_last_bb_exprs = {
                    let mut then_gen = BlockGenerator::new(self.func_gen);
                    then_gen.gen_stat(result, then);
                    then_gen.exprs
                };
                let then_last_bb_id = self.func_gen.bbs.push_and_get_key(BasicBlockOptionalNext {
                    exprs: then_last_bb_exprs,
                    next: None,
                });

                let else_first_bb_id = self.func_gen.bbs.next_key();
                let else_bb_exprs = {
                    let mut els_gen = BlockGenerator::new(self.func_gen);
                    els_gen.gen_stat(result, els);
                    els_gen.exprs
                };
                let else_last_bb_id = self.func_gen.bbs.push_and_get_key(BasicBlockOptionalNext {
                    exprs: else_bb_exprs,
                    next: None,
                });

                self.func_gen.bbs[bb_id].next = Some(BasicBlockNext::If(
                    cond_local,
                    then_first_bb_id,
                    else_first_bb_id,
                ));

                self.func_gen.next_undecided_bb_ids.insert(then_last_bb_id);
                self.func_gen.next_undecided_bb_ids.insert(else_last_bb_id);
            }
            ast::Expr::Call(x, ast::Call { func, args }) => {
                if let ast::Expr::Var(x, _) = func.as_ref()
                    && let ast::UsedVarR {
                        var_id: ast::VarId::Builtin(builtin),
                    } = x.get_ref(type_map::key::<Used>())
                {
                    let builtin_typ = builtin_func_type(*builtin);
                    debug_assert!(builtin_typ.rets.len() == 1);
                    let ret_type = builtin_typ.rets[0];
                    if builtin_typ.args.len() != args.len() {
                        let msg = self.func_gen.local(Type::Val(ValType::String));
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
                            let boxed_arg_local = self.func_gen.local(Type::Boxed);
                            self.gen_stat(Some(boxed_arg_local), arg);
                            let arg_local = match typ {
                                Type::Boxed => boxed_arg_local,
                                Type::Val(val_type) => {
                                    let unboxed_arg_local =
                                        self.func_gen.local(Type::Val(*val_type));
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
                            Type::Boxed => self.func_gen.local(Type::Boxed),
                            Type::Val(val_type) => self.func_gen.local(Type::Val(val_type)),
                        };
                        self.exprs.push(ExprAssign {
                            local: Some(ret_local),
                            expr: Expr::Builtin(*builtin, arg_locals),
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
                    let boxed_func_local = self.func_gen.local(Type::Boxed);
                    self.gen_stat(Some(boxed_func_local), func);

                    // TODO: funcがクロージャかのチェック
                    let func_local = self.func_gen.local(Type::Val(ValType::Closure));
                    self.exprs.push(ExprAssign {
                        local: Some(func_local),
                        expr: Expr::Unbox(ValType::Closure, boxed_func_local),
                    });

                    // TODO: 引数の数が合っているかのチェック
                    let mut arg_locals = Vec::new();
                    arg_locals.push(func_local); // 第一引数にクロージャを渡す
                    for arg in args {
                        let arg_local = self.func_gen.local(Type::Boxed);
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
                    if self.func_gen.ir_generator.box_vars.contains(id) {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::DerefMutCell(*self.func_gen.local_ids.get(id).unwrap()),
                        });
                    } else {
                        self.exprs.push(ExprAssign {
                            local: result,
                            expr: Expr::Move(*self.func_gen.local_ids.get(id).unwrap()),
                        });
                    }
                }
                ast::VarId::Global(id) => {
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::GlobalGet(id.0),
                    });
                }
                ast::VarId::Builtin(builtin) => {
                    self.exprs.push(ExprAssign {
                        local: result,
                        expr: Expr::GetBuiltin(*builtin),
                    });
                }
            },
            ast::Expr::Begin(_, ast::Begin { exprs: stats }) => {
                self.gen_stats(result, stats);
            }
            ast::Expr::Set(x, ast::Set { expr, .. }) => {
                match &x.get_ref(type_map::key::<Used>()).var_id {
                    ast::VarId::Local(id) => {
                        if self.func_gen.ir_generator.box_vars.contains(id) {
                            let boxed_local = self.func_gen.local(Type::Boxed);
                            self.gen_stat(Some(boxed_local), expr);
                            let local = self.func_gen.local_ids.get(id).unwrap();
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::SetMutCell(*local, boxed_local),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(boxed_local),
                            });
                        } else {
                            let local = *self.func_gen.local_ids.get(id).unwrap();
                            self.gen_stat(Some(local), expr);
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(local),
                            });
                        }
                    }
                    ast::VarId::Global(id) => {
                        let local = self.func_gen.local(Type::Boxed);
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
                    ast::VarId::Builtin(builtin) => {
                        if self.func_gen.ir_generator.config.allow_set_builtin {
                            let local = self.func_gen.local(Type::Boxed);
                            self.gen_stat(Some(local), expr);
                            self.exprs.push(ExprAssign {
                                local: None,
                                expr: Expr::SetBuiltin(*builtin, local),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Move(local),
                            });
                        } else {
                            let msg = self.func_gen.local(Type::Val(ValType::String));
                            self.exprs.push(ExprAssign {
                                local: Some(msg),
                                expr: Expr::String("set! builtin is not allowed\n".to_string()),
                            });
                            self.exprs.push(ExprAssign {
                                local: result,
                                expr: Expr::Error(msg),
                            });
                        }
                    }
                }
            }
            ast::Expr::Let(x, _) => *x.get_ref(type_map::key::<Desugared>()),
        }
    }

    fn quote(&mut self, result: Option<usize>, sexpr: &sexpr::SExpr) {
        match &sexpr.kind {
            sexpr::SExprKind::Bool(b) => {
                let unboxed = self.func_gen.local(Type::Val(ValType::Bool));
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
                let unboxed = self.func_gen.local(Type::Val(ValType::Int));
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
                let unboxed = self.func_gen.local(Type::Val(ValType::String));
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
                let string = self.func_gen.local(Type::Val(ValType::String));
                let unboxed = self.func_gen.local(Type::Val(ValType::Symbol));
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
                let unboxed = self.func_gen.local(Type::Val(ValType::Nil));
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
                let unboxed = self.func_gen.local(Type::Val(ValType::Char));
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
                let car_local = self.func_gen.local(Type::Boxed);
                self.quote(Some(car_local), &cons.car);
                let cdr_local = self.func_gen.local(Type::Boxed);
                self.quote(Some(cdr_local), &cons.cdr);

                let unboxed = self.func_gen.local(Type::Val(ValType::Cons));
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

    fn gen_stats(&mut self, result: Option<usize>, stats: &[ast::Expr<ast::Final>]) {
        if let Some((last, rest)) = stats.split_last() {
            for stat in rest {
                self.gen_stat(None, stat);
            }
            self.gen_stat(result, last);
        } else {
            let unboxed = self.func_gen.local(Type::Val(ValType::Nil));
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
        let bb_id = self.func_gen.bbs.push_and_get_key(BasicBlockOptionalNext {
            exprs: bb_exprs,
            next,
        });

        let undecided_bb_ids = std::mem::take(&mut self.func_gen.next_undecided_bb_ids);
        for undecided_bb_id in undecided_bb_ids {
            self.func_gen.bbs[undecided_bb_id].next = Some(BasicBlockNext::Jump(bb_id));
        }

        bb_id
    }
}

pub fn builtin_func_type(builtin: ast::Builtin) -> FuncType {
    match builtin {
        ast::Builtin::Display => FuncType {
            args: vec![Type::Val(ValType::String)], // TODO: 一旦Stringのみ
            rets: vec![Type::Val(ValType::Nil)],
        },
        ast::Builtin::Add => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Int)],
        },
        ast::Builtin::Sub => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Int)],
        },
        ast::Builtin::WriteChar => FuncType {
            args: vec![Type::Val(ValType::Char)],
            rets: vec![Type::Val(ValType::Nil)],
        },
        ast::Builtin::IsPair => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::IsSymbol => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::IsString => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::IsNumber => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::IsBoolean => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::IsProcedure => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::IsChar => FuncType {
            args: vec![Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::Eq => FuncType {
            args: vec![Type::Boxed, Type::Boxed],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::Car => FuncType {
            args: vec![Type::Val(ValType::Cons)],
            rets: vec![Type::Boxed],
        },
        ast::Builtin::Cdr => FuncType {
            args: vec![Type::Val(ValType::Cons)],
            rets: vec![Type::Boxed],
        },
        ast::Builtin::SymbolToString => FuncType {
            args: vec![Type::Val(ValType::Symbol)],
            rets: vec![Type::Val(ValType::String)],
        },
        ast::Builtin::NumberToString => FuncType {
            args: vec![Type::Val(ValType::Int)], // TODO: 一般のnumberに使えるように
            rets: vec![Type::Val(ValType::String)],
        },
        ast::Builtin::EqNum => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::Lt => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::Gt => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::Le => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Bool)],
        },
        ast::Builtin::Ge => FuncType {
            args: vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
            rets: vec![Type::Val(ValType::Bool)],
        },
    }
}
