use super::Desugared;
use super::TailCall;
use super::astx::*;
use super::defined::*;
use crate::x::FamilyX;
use crate::x::Phase;
use crate::x::TypeMap;
use crate::x::type_map;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone)]
pub enum Used {}

impl Phase for Used {
    type Prev = TailCall;
}

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
    pub box_vars: FxHashSet<LocalVarId>,
    pub global_vars: FxHashSet<GlobalVarId>,
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
    type R = ();
}

impl FamilyX<Used> for DefineX {
    type R = ();
}

impl FamilyX<Used> for LambdaX {
    type R = UsedLambdaR;
}

impl FamilyX<Used> for IfX {
    type R = ();
}

impl FamilyX<Used> for CallX {
    type R = ();
}

impl FamilyX<Used> for VarX {
    type R = UsedVarR;
}

impl FamilyX<Used> for BeginX {
    type R = ();
}

impl FamilyX<Used> for SetX {
    type R = UsedSetR;
}

impl FamilyX<Used> for LetX {
    type R = ();
}

#[derive(Debug, Clone)]
enum Context {
    Global,
    Local(LocalContext),
}

#[derive(Debug, Clone)]
struct LocalContext {
    env: FxHashMap<String, EnvLocalVar>,
}

#[derive(Debug, Clone)]
struct EnvLocalVar {
    id: LocalVarId,
    is_captured: bool,
}

#[derive(Debug, Clone)]
struct State {
    captures: FxHashSet<LocalVarId>,
}

impl State {
    fn new() -> Self {
        State {
            captures: FxHashSet::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct VarIdGen {
    global_count: usize,
    local_count: usize,
    globals: FxHashMap<String, GlobalVarId>,
    mutated_vars: FxHashSet<LocalVarId>,
    captured_vars: FxHashSet<LocalVarId>,
    // 以下はモジュールごとにリセットされる状態
    // このモジュールで使ったグローバル変数
    use_globals: FxHashSet<GlobalVarId>,
    pub local_metas: FxHashMap<LocalVarId, VarMeta>,
    pub global_metas: FxHashMap<GlobalVarId, VarMeta>,
}

impl Default for VarIdGen {
    fn default() -> Self {
        Self::new()
    }
}

impl VarIdGen {
    pub fn new() -> Self {
        VarIdGen {
            global_count: 0,
            local_count: 0,
            globals: FxHashMap::default(),
            mutated_vars: FxHashSet::default(),
            captured_vars: FxHashSet::default(),
            use_globals: FxHashSet::default(),
            local_metas: FxHashMap::default(),
            global_metas: FxHashMap::default(),
        }
    }

    fn gen_global(&mut self, meta: VarMeta) -> GlobalVarId {
        let id = self.global_count;
        self.global_count += 1;
        let id = GlobalVarId(id);
        self.global_metas.insert(id, meta);
        id
    }

    fn gen_local(&mut self, meta: VarMeta) -> LocalVarId {
        let id = self.local_count;
        self.local_count += 1;
        let id = LocalVarId(id);
        self.local_metas.insert(id, meta);
        id
    }

    fn global_var_id(&mut self, name: &str) -> GlobalVarId {
        let id = if let Some(id) = self.globals.get(name) {
            *id
        } else {
            let id = self.gen_global(VarMeta {
                name: name.to_string(),
            });
            self.globals.insert(name.to_string(), id);
            id
        };
        self.use_globals.insert(id);
        id
    }

    pub fn get_global_id(&self, name: &str) -> Option<GlobalVarId> {
        self.globals.get(name).copied()
    }

    fn flag_mutate(&mut self, id: LocalVarId) {
        self.mutated_vars.insert(id);
    }

    fn flag_capture(&mut self, id: LocalVarId) {
        self.captured_vars.insert(id);
    }

    fn reset_for_module(&mut self) {
        self.use_globals.clear();
    }
}

impl Ast<Used> {
    pub fn from_ast(ast: Ast<<Used as Phase>::Prev>, var_id_gen: &mut VarIdGen) -> Self {
        var_id_gen.reset_for_module();
        let new_exprs = ast
            .exprs
            .into_iter()
            .map(|expr| Expr::from_expr(expr, &Context::Global, var_id_gen, &mut State::new()))
            .collect();

        Ast {
            x: ast.x.add(type_map::key::<Used>(), UsedAstR {
                box_vars: var_id_gen
                    .mutated_vars
                    .intersection(&var_id_gen.captured_vars)
                    .copied()
                    .collect(),
                global_vars: var_id_gen.use_globals.clone(),
            }),
            exprs: new_exprs,
        }
    }
}

impl Expr<Used> {
    fn from_expr(
        expr: Expr<<Used as Phase>::Prev>,
        ctx: &Context,
        var_id_gen: &mut VarIdGen,
        state: &mut State,
    ) -> Self {
        match expr {
            Expr::Literal(x, lit) => Expr::Literal(x.add(type_map::key::<Used>(), ()), lit),
            Expr::Var(x, var) => {
                let var_id = match ctx {
                    Context::Global => VarId::Global(var_id_gen.global_var_id(&var)),
                    Context::Local(LocalContext { env }) => {
                        if let Some(local_var) = env.get(&var) {
                            if local_var.is_captured {
                                state.captures.insert(local_var.id);
                            }
                            VarId::Local(local_var.id)
                        } else {
                            VarId::Global(var_id_gen.global_var_id(&var))
                        }
                    }
                };
                Expr::Var(x.add(type_map::key::<Used>(), UsedVarR { var_id }), var)
            }
            Expr::Define(x, _) => x.get_owned(type_map::key::<Defined>()),
            Expr::Lambda(x, lambda) => {
                let mut new_env = FxHashMap::default();
                match ctx {
                    Context::Global => {}
                    Context::Local(LocalContext { env }) => {
                        for (name, local_var) in env.iter() {
                            new_env.insert(name.clone(), EnvLocalVar {
                                id: local_var.id,
                                is_captured: true,
                            });
                        }
                    }
                }
                let args = lambda
                    .args
                    .iter()
                    .map(|arg| {
                        let id = var_id_gen.gen_local(VarMeta { name: arg.clone() });
                        new_env.insert(arg.clone(), EnvLocalVar {
                            id,
                            is_captured: false,
                        });
                        id
                    })
                    .collect::<Vec<_>>();

                let defines: Vec<_> = x
                    .get_ref(type_map::key::<Defined>())
                    .defines
                    .iter()
                    .map(|def| {
                        let id = var_id_gen.gen_local(VarMeta { name: def.clone() });
                        new_env.insert(def.clone(), EnvLocalVar {
                            id,
                            is_captured: false,
                        });
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

                for free_var in new_state.captures.iter() {
                    var_id_gen.flag_capture(*free_var);
                }

                Expr::Lambda(
                    x.add(type_map::key::<Used>(), UsedLambdaR {
                        args,
                        defines,
                        captures: new_state.captures.into_iter().collect(), // 非決定的だが問題ないはず
                    }),
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
                Expr::If(x.add(type_map::key::<Used>(), ()), If {
                    cond: new_cond,
                    then: new_then,
                    els: new_els,
                })
            }
            Expr::Call(x, call) => {
                let new_func = Box::new(Self::from_expr(*call.func, ctx, var_id_gen, state));
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_expr(arg, ctx, var_id_gen, state))
                    .collect();
                Expr::Call(x.add(type_map::key::<Used>(), ()), Call {
                    func: new_func,
                    args: new_args,
                })
            }
            Expr::Begin(x, begin) => {
                let new_exprs = begin
                    .exprs
                    .into_iter()
                    .map(|expr| Self::from_expr(expr, ctx, var_id_gen, state))
                    .collect();
                Expr::Begin(x.add(type_map::key::<Used>(), ()), Begin {
                    exprs: new_exprs,
                })
            }
            Expr::Set(x, set) => {
                let var_id = match ctx {
                    Context::Global => VarId::Global(var_id_gen.global_var_id(&set.name)),
                    Context::Local(LocalContext { env }) => {
                        if let Some(local_var) = env.get(&set.name) {
                            if local_var.is_captured {
                                state.captures.insert(local_var.id);
                            }
                            var_id_gen.flag_mutate(local_var.id);
                            VarId::Local(local_var.id)
                        } else {
                            VarId::Global(var_id_gen.global_var_id(&set.name))
                        }
                    }
                };
                let new_expr = Self::from_expr(*set.expr, ctx, var_id_gen, state);
                Expr::Set(x.add(type_map::key::<Used>(), UsedSetR { var_id }), Set {
                    name: set.name,
                    expr: Box::new(new_expr),
                })
            }
            Expr::Let(x, _) => x.get_owned(type_map::key::<Desugared>()),
        }
    }
}
