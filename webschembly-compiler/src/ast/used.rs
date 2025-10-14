use super::astx::*;
use crate::ast::TailCallCallR;
use crate::x::FamilyX;
use crate::x::Phase;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone)]
pub enum Used {}

impl Phase for Used {}

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
    pub local_metas: FxHashMap<LocalVarId, VarMeta>,
    pub global_metas: FxHashMap<GlobalVarId, VarMeta>,
    pub defines: Vec<LocalVarId>,
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
impl FamilyX<Used> for ConstX {
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

// tail_callから引き継ぐためのものだが、もう少し綺麗なやり方を考えたい
#[derive(Debug, Clone)]
pub struct UsedCallR {
    pub is_tail: bool,
}

impl FamilyX<Used> for CallX {
    type R = UsedCallR;
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
    type R = !;
}

impl FamilyX<Used> for LetRecX {
    type R = !;
}

impl FamilyX<Used> for VectorX {
    type R = ();
}
impl FamilyX<Used> for UVectorX {
    type R = ();
}
impl FamilyX<Used> for QuoteX {
    type R = ();
}

impl FamilyX<Used> for ConsX {
    type R = ();
}

pub trait UsedPrevPhase = XBound
where
    DefineX: FamilyX<Self, R = !>,
    BeginX: FamilyX<Self, R = !>,
    QuoteX: FamilyX<Self, R = !>,
    CallX: FamilyX<Self, R = TailCallCallR>;

type SelfExpr = Expr<Used>;

#[derive(Debug, Clone)]
struct Context {
    env: FxHashMap<String, EnvLocalVar>,
}

impl Context {
    fn new_empty() -> Self {
        Context {
            env: FxHashMap::default(),
        }
    }

    fn extend_some_lambda(
        mut self,
        new_env: impl IntoIterator<Item = (String, EnvLocalVar)>,
    ) -> Self {
        for (name, local_var) in new_env {
            self.env.insert(name, local_var);
        }
        self
    }

    fn extend_new_lambda(
        mut self,
        new_env: impl IntoIterator<Item = (String, EnvLocalVar)>,
    ) -> Self {
        for (_, var) in self.env.iter_mut() {
            var.is_captured = true;
        }

        self.extend_some_lambda(new_env)
    }
}

#[derive(Debug, Clone)]
struct EnvLocalVar {
    id: LocalVarId,
    is_captured: bool,
}

#[derive(Debug, Clone)]
struct LambdaState {
    captures: FxHashSet<LocalVarId>,
    defines: Vec<LocalVarId>,
}

impl LambdaState {
    fn new() -> Self {
        LambdaState {
            captures: FxHashSet::default(),
            defines: Vec::new(),
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
    local_metas: FxHashMap<LocalVarId, VarMeta>,
    global_metas: FxHashMap<GlobalVarId, VarMeta>,
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
    pub fn from_ast<P: UsedPrevPhase>(ast: Ast<P>, var_id_gen: &mut VarIdGen) -> Self {
        var_id_gen.reset_for_module();
        let mut defines = Vec::new();
        let mut result = Vec::new();
        for expr in ast.exprs {
            let mut state = LambdaState::new();
            LExpr::from_expr(
                expr,
                &Context::new_empty(),
                var_id_gen,
                &mut state,
                &mut result,
            );
            debug_assert!(state.captures.is_empty());
            defines.extend(state.defines);
        }

        Ast {
            x: UsedAstR {
                box_vars: var_id_gen
                    .mutated_vars
                    .intersection(&var_id_gen.captured_vars)
                    .copied()
                    .collect(),
                global_vars: var_id_gen.use_globals.clone(),
                local_metas: var_id_gen.local_metas.clone(),
                global_metas: var_id_gen.global_metas.clone(),
                defines,
            },
            exprs: result,
        }
    }
}

impl LExpr<Used> {
    fn from_expr<P: UsedPrevPhase>(
        expr: LExpr<P>,
        ctx: &Context,
        var_id_gen: &mut VarIdGen,
        state: &mut LambdaState,
        result: &mut Vec<LExpr<Used>>,
    ) {
        match expr.value {
            Expr::Const(_, lit) => result.push(SelfExpr::Const((), lit).with_span(expr.span)),
            Expr::Var(_, var) => {
                let var_id = if let Some(local_var) = ctx.env.get(&var) {
                    if local_var.is_captured {
                        state.captures.insert(local_var.id);
                    }
                    VarId::Local(local_var.id)
                } else {
                    VarId::Global(var_id_gen.global_var_id(&var))
                };
                result.push(SelfExpr::Var(UsedVarR { var_id }, var).with_span(expr.span))
            }
            Expr::Define(x, _) => x,
            Expr::Lambda(_, lambda) => {
                let mut new_env = FxHashMap::default();

                let args = lambda
                    .args
                    .iter()
                    .map(|Located { value: arg, .. }| {
                        let id = var_id_gen.gen_local(VarMeta { name: arg.clone() });
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

                let mut new_state = LambdaState::new();
                let new_ctx = ctx.clone().extend_new_lambda(new_env);
                let new_body = Self::from_exprs(lambda.body, &new_ctx, var_id_gen, &mut new_state);

                {
                    // キャプチャリストを親ラムダが継承する
                    // ただし、親ラムダで定義されている変数を除く
                    let mut exnted_captures = new_state.captures.clone();
                    for var in ctx.env.values() {
                        if !var.is_captured {
                            exnted_captures.remove(&var.id);
                        }
                    }
                    state.captures.extend(exnted_captures);
                }

                // 一度でもキャプチャされた変数はref化の必要がある可能性があるのでフラグをつける
                for free_var in new_state.captures.iter() {
                    var_id_gen.flag_capture(*free_var);
                }

                result.push(
                    SelfExpr::Lambda(
                        UsedLambdaR {
                            args,
                            defines: new_state.defines,
                            captures: new_state.captures.into_iter().collect(), // 非決定的だが問題ないはず
                        },
                        Lambda {
                            args: lambda.args,
                            body: new_body,
                        },
                    )
                    .with_span(expr.span),
                );
            }
            Expr::If(_, if_) => {
                let new_cond = Self::from_exprs(if_.cond, ctx, var_id_gen, state);
                let new_then = Self::from_exprs(if_.then, ctx, var_id_gen, state);
                let new_els = Self::from_exprs(if_.els, ctx, var_id_gen, state);
                result.push(
                    SelfExpr::If(
                        (),
                        If {
                            cond: new_cond,
                            then: new_then,
                            els: new_els,
                        },
                    )
                    .with_span(expr.span),
                )
            }
            Expr::Call(x, call) => {
                let new_func = Self::from_exprs(call.func, ctx, var_id_gen, state);
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_exprs(arg, ctx, var_id_gen, state))
                    .collect();
                result.push(
                    SelfExpr::Call(
                        UsedCallR { is_tail: x.is_tail },
                        Call {
                            func: new_func,
                            args: new_args,
                        },
                    )
                    .with_span(expr.span),
                )
            }
            Expr::Begin(x, _) => x,
            Expr::Set(_, set) => {
                let var_id = if let Some(local_var) = ctx.env.get(&set.name.value) {
                    if local_var.is_captured {
                        state.captures.insert(local_var.id);
                    }
                    var_id_gen.flag_mutate(local_var.id);
                    VarId::Local(local_var.id)
                } else {
                    VarId::Global(var_id_gen.global_var_id(&set.name.value))
                };
                let new_expr = Self::from_exprs(set.expr, ctx, var_id_gen, state);
                result.push(
                    SelfExpr::Set(
                        UsedSetR { var_id },
                        Set {
                            name: set.name,
                            expr: new_expr,
                        },
                    )
                    .with_span(expr.span),
                )
            }
            Expr::Let(_, let_) => {
                let mut new_env = FxHashMap::default();

                for Located {
                    value: Binding { name, expr },
                    span: binding_span,
                } in let_.bindings.iter()
                {
                    let id = var_id_gen.gen_local(VarMeta {
                        name: name.value.clone(),
                    });
                    new_env.insert(
                        name.value.clone(),
                        EnvLocalVar {
                            id,
                            is_captured: false,
                        },
                    );
                    state.defines.push(id);

                    let expr = Self::from_exprs(expr.clone(), ctx, var_id_gen, state);
                    let set_expr = SelfExpr::Set(
                        UsedSetR {
                            var_id: VarId::Local(id),
                        },
                        Set {
                            name: name.clone(),
                            expr,
                        },
                    )
                    .with_span(*binding_span);
                    result.push(set_expr);
                }

                let new_ctx = ctx.clone().extend_some_lambda(new_env);

                for expr in let_.body {
                    // stateは親のものを引き継ぐ
                    Self::from_expr(expr, &new_ctx, var_id_gen, state, result);
                }
            }
            Expr::LetRec(_, letrec) => {
                // TODO: letrecの定義式で同じletrecの変数を参照する場合、ラムダで囲わないとエラーにする必要がある
                // (letrec ((a 1) (b (+ a 1))) b) のようなものは許されない。let*を使うべき

                let mut new_env = FxHashMap::default();
                let mut ids = Vec::new();
                for Located {
                    value: Binding { name, .. },
                    ..
                } in letrec.bindings.iter()
                {
                    let id = var_id_gen.gen_local(VarMeta {
                        name: name.value.clone(),
                    });
                    // letrecはflag_mutateが必要だが、1度しか代入されない場合特殊化したい
                    var_id_gen.flag_mutate(id);
                    new_env.insert(
                        name.value.clone(),
                        EnvLocalVar {
                            id,
                            is_captured: false,
                        },
                    );
                    state.defines.push(id);
                    ids.push(id);
                }

                let ctx = ctx.clone().extend_some_lambda(new_env);
                for (
                    Located {
                        value: Binding { name, expr },
                        span: binding_span,
                    },
                    &id,
                ) in letrec.bindings.iter().zip(&ids)
                {
                    let expr = Self::from_exprs(expr.clone(), &ctx, var_id_gen, state);
                    let set_expr = SelfExpr::Set(
                        UsedSetR {
                            var_id: VarId::Local(id),
                        },
                        Set {
                            name: name.clone(),
                            expr,
                        },
                    )
                    .with_span(*binding_span);
                    result.push(set_expr);
                }

                for expr in letrec.body.into_iter() {
                    // stateは親のものを引き継ぐ
                    Self::from_expr(expr, &ctx, var_id_gen, state, result);
                }
            }
            Expr::Vector(_, vec) => {
                let new_vec = vec
                    .into_iter()
                    .map(|expr| Self::from_exprs(expr, ctx, var_id_gen, state))
                    .collect();
                result.push(SelfExpr::Vector((), new_vec).with_span(expr.span))
            }
            Expr::UVector(_, uvec) => result.push(
                SelfExpr::UVector(
                    (),
                    UVector {
                        kind: uvec.kind,
                        elements: uvec
                            .elements
                            .into_iter()
                            .map(|expr| Self::from_exprs(expr, ctx, var_id_gen, state))
                            .collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Quote(x, _) => x,
            Expr::Cons(_, cons) => {
                let new_car = Self::from_exprs(cons.car, ctx, var_id_gen, state);
                let new_cdr = Self::from_exprs(cons.cdr, ctx, var_id_gen, state);
                result.push(
                    SelfExpr::Cons(
                        (),
                        Cons {
                            car: new_car,
                            cdr: new_cdr,
                        },
                    )
                    .with_span(expr.span),
                );
            }
        }
    }

    fn from_exprs<P: UsedPrevPhase>(
        exprs: Vec<LExpr<P>>,
        ctx: &Context,
        var_id_gen: &mut VarIdGen,
        state: &mut LambdaState,
    ) -> Vec<LExpr<Used>> {
        let mut result = Vec::new();
        for expr in exprs {
            Self::from_expr(expr, ctx, var_id_gen, state, &mut result);
        }
        result
    }
}
