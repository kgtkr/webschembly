use std::collections::HashMap;
use std::collections::HashSet;

use super::ast::*;
use super::defined::*;
use crate::x::FamilyX;

/*
名前付き変数にモジュール内で一意なIDを割り振る
変数の使用を解析する
* ラムダ式でキャプチャするべき変数の決定
* set! されている変数をBox化する

TODO:
定義したラムダ式と同じラムダ式からのみset!される変数はBox化しなくても良い
*/

#[derive(Debug, Clone)]
pub enum Used {}
type Prev = Defined;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalVarId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalVarId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarId {
    Global(GlobalVarId),
    Local(LocalVarId),
}

#[derive(Debug, Clone)]
pub struct UsedAstR {
    // TODO: 現在未使用
    pub box_vars: HashSet<LocalVarId>,
    pub global_vars: HashSet<GlobalVarId>,
}

#[derive(Debug, Clone)]
pub struct UsedLambdaR {
    pub args: Vec<LocalVarId>,
    pub defines: Vec<LocalVarId>,
    pub captures: Vec<LocalVarId>,
}

#[derive(Debug, Clone)]
pub struct UsedVarR {
    pub var_id: VarId,
}

#[derive(Debug, Clone)]
pub struct UsedSetR {
    pub var_id: VarId,
}

impl FamilyX<Used> for AstX {
    type R = UsedAstR;
}
impl FamilyX<Used> for LiteralX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Used> for DefineX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Used> for LambdaX {
    type R = UsedLambdaR;
}

impl FamilyX<Used> for IfX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Used> for CallX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Used> for VarX {
    type R = UsedVarR;
}

impl FamilyX<Used> for BeginX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Used> for DumpX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Used> for SetX {
    type R = UsedSetR;
}

#[derive(Debug, Clone)]
enum Context {
    Global,
    Local(LocalContext),
}

#[derive(Debug, Clone)]
struct LocalContext {
    env: HashMap<String, EnvLocalVar>,
}

#[derive(Debug, Clone)]
struct EnvLocalVar {
    id: LocalVarId,
    is_captured: bool,
}

#[derive(Debug, Clone)]
struct State {
    captures: HashSet<LocalVarId>,
}

impl State {
    fn new() -> Self {
        State {
            captures: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct VarIdGen {
    global: usize,
    local: usize,
    globals: HashMap<String, GlobalVarId>,
    mutates: HashSet<LocalVarId>,
}

impl VarIdGen {
    fn new() -> Self {
        VarIdGen {
            global: 0,
            local: 0,
            globals: HashMap::new(),
            mutates: HashSet::new(),
        }
    }

    fn gen_global(&mut self) -> GlobalVarId {
        let id = self.global;
        self.global += 1;
        GlobalVarId(id)
    }

    fn gen_local(&mut self) -> LocalVarId {
        let id = self.local;
        self.local += 1;
        LocalVarId(id)
    }

    fn get_global(&mut self, name: &str) -> GlobalVarId {
        if let Some(id) = self.globals.get(name) {
            *id
        } else {
            let id = self.gen_global();
            self.globals.insert(name.to_string(), id);
            id
        }
    }

    fn flag_mutate(&mut self, id: LocalVarId) {
        self.mutates.insert(id);
    }
}

impl Ast<Used> {
    pub fn from_ast(ast: Ast<Prev>) -> Self {
        let mut var_id_gen = VarIdGen::new();
        let new_exprs = ast
            .exprs
            .into_iter()
            .map(|expr| Expr::from_expr(expr, &Context::Global, &mut var_id_gen, &mut State::new()))
            .collect();

        Ast {
            x: UsedAstR {
                box_vars: var_id_gen.mutates,
                global_vars: var_id_gen.globals.values().copied().collect(),
            },
            exprs: new_exprs,
        }
    }
}

impl Expr<Used> {
    fn from_expr(
        expr: Expr<Prev>,
        ctx: &Context,
        var_id_gen: &mut VarIdGen,
        state: &mut State,
    ) -> Self {
        match expr {
            Expr::Literal(x, lit) => Expr::Literal(x, lit),
            Expr::Var(_, var) => {
                let var_id = match ctx {
                    Context::Global => VarId::Global(var_id_gen.get_global(&var)),
                    Context::Local(LocalContext { env }) => {
                        if let Some(local_var) = env.get(&var) {
                            if local_var.is_captured {
                                state.captures.insert(local_var.id);
                            }
                            VarId::Local(local_var.id)
                        } else {
                            VarId::Global(var_id_gen.get_global(&var))
                        }
                    }
                };
                Expr::Var(UsedVarR { var_id }, var)
            }
            Expr::Define(x, _) => x,
            Expr::Lambda(x, lambda) => {
                let mut new_env = HashMap::new();
                match ctx {
                    Context::Global => {}
                    Context::Local(LocalContext { env }) => {
                        for (name, local_var) in env.iter() {
                            new_env.insert(
                                name.clone(),
                                EnvLocalVar {
                                    id: local_var.id,
                                    is_captured: true,
                                },
                            );
                        }
                    }
                }
                let args = lambda
                    .args
                    .iter()
                    .map(|arg| {
                        let id = var_id_gen.gen_local();
                        new_env.insert(
                            arg.clone(),
                            EnvLocalVar {
                                id,
                                is_captured: false,
                            },
                        );
                        id
                    })
                    .collect::<Vec<_>>();
                let defines: Vec<_> = x
                    .defines
                    .iter()
                    .map(|def| {
                        let id = var_id_gen.gen_local();
                        new_env.insert(
                            def.clone(),
                            EnvLocalVar {
                                id,
                                is_captured: false,
                            },
                        );
                        id
                    })
                    .collect();
                let mut new_state = State::new();
                let new_ctx = Context::Local(LocalContext { env: new_env });
                let new_body = lambda
                    .body
                    .into_iter()
                    .map(|expr| Self::from_expr(expr, &new_ctx, var_id_gen, &mut new_state))
                    .collect::<Vec<_>>();

                // free_vars = new_captures - args - defines
                for arg in &args {
                    new_state.captures.remove(arg);
                }
                for def in &defines {
                    new_state.captures.remove(def);
                }

                state.captures.extend(new_state.captures.iter().copied());

                Expr::Lambda(
                    UsedLambdaR {
                        args: args,
                        defines: defines,
                        captures: new_state.captures.into_iter().collect(), // 非決定的だが問題ないはず
                    },
                    Lambda {
                        args: lambda.args,
                        body: new_body,
                    },
                )
            }
            Expr::If(x, if_) => {
                let new_cond = Box::new(Self::from_expr(*if_.cond, ctx, var_id_gen, state));
                let new_then = Box::new(Self::from_expr(*if_.then, ctx, var_id_gen, state));
                let new_els = Box::new(Self::from_expr(*if_.els, ctx, var_id_gen, state));
                Expr::If(
                    x,
                    If {
                        cond: new_cond,
                        then: new_then,
                        els: new_els,
                    },
                )
            }
            Expr::Call(x, call) => {
                let new_func = Box::new(Self::from_expr(*call.func, ctx, var_id_gen, state));
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_expr(arg, ctx, var_id_gen, state))
                    .collect();
                Expr::Call(
                    x,
                    Call {
                        func: new_func,
                        args: new_args,
                    },
                )
            }
            Expr::Begin(x, begin) => {
                let new_exprs = begin
                    .exprs
                    .into_iter()
                    .map(|expr| Self::from_expr(expr, ctx, var_id_gen, state))
                    .collect();
                Expr::Begin(x, Begin { exprs: new_exprs })
            }
            Expr::Dump(x, dump) => {
                let new_expr = Box::new(Self::from_expr(*dump, ctx, var_id_gen, state));
                Expr::Dump(x, new_expr)
            }
            Expr::Set(_, set) => {
                let var_id = match ctx {
                    Context::Global => VarId::Global(var_id_gen.get_global(&set.name)),
                    Context::Local(LocalContext { env }) => {
                        let local_var = env.get(&set.name).unwrap();
                        var_id_gen.flag_mutate(local_var.id);
                        VarId::Local(local_var.id)
                    }
                };
                let new_expr = Self::from_expr(*set.expr, ctx, var_id_gen, state);
                Expr::Set(
                    UsedSetR { var_id },
                    Set {
                        name: set.name,
                        expr: Box::new(new_expr),
                    },
                )
            }
        }
    }
}
