use super::astx::*;
use crate::x::FamilyX;
use crate::x::Phase;

#[derive(Debug, Clone)]
pub enum TailCall {}

impl Phase for TailCall {}

impl FamilyX<TailCall> for AstX {
    type R = ();
}
impl FamilyX<TailCall> for ConstX {
    type R = ();
}
impl FamilyX<TailCall> for DefineX {
    type R = !;
}
impl FamilyX<TailCall> for LambdaX {
    type R = ();
}
impl FamilyX<TailCall> for IfX {
    type R = ();
}

#[derive(Debug, Clone)]
pub struct TailCallCallR {
    pub is_tail: bool,
}

impl FamilyX<TailCall> for CallX {
    type R = TailCallCallR;
}
impl FamilyX<TailCall> for VarX {
    type R = ();
}
impl FamilyX<TailCall> for BeginX {
    type R = !;
}
impl FamilyX<TailCall> for SetX {
    type R = ();
}
impl FamilyX<TailCall> for LetX {
    type R = ();
}
impl FamilyX<TailCall> for LetRecX {
    type R = ();
}
impl FamilyX<TailCall> for VectorX {
    type R = ();
}
impl FamilyX<TailCall> for UVectorX {
    type R = ();
}
impl FamilyX<TailCall> for QuoteX {
    type R = !;
}

impl FamilyX<TailCall> for ConsX {
    type R = ();
}

pub trait TailCallPrevPhase = XBound
where
    DefineX: FamilyX<Self, R = !>,
    BeginX: FamilyX<Self, R = !>,
    QuoteX: FamilyX<Self, R = !>;
type SelfExpr = Expr<TailCall>;

impl Ast<TailCall> {
    pub fn from_ast<P: TailCallPrevPhase>(ast: Ast<P>) -> Self {
        Ast {
            x: (),
            exprs: ast
                .exprs
                .into_iter()
                .map(|expr| LExpr::from_expr(expr, false))
                .collect(),
        }
    }
}

impl LExpr<TailCall> {
    fn from_expr<P: TailCallPrevPhase>(expr: LExpr<P>, is_tail: bool) -> Self {
        match expr.value {
            Expr::Const(_, lit) => SelfExpr::Const((), lit).with_span(expr.span),
            Expr::Var(_, var) => SelfExpr::Var((), var).with_span(expr.span),
            Expr::Define(x, _) => x,
            Expr::Lambda(_, lambda) => SelfExpr::Lambda(
                (),
                Lambda {
                    args: lambda.args,
                    body: Self::from_exprs(lambda.body, true),
                },
            )
            .with_span(expr.span),
            Expr::If(_, if_) => SelfExpr::If(
                (),
                If {
                    cond: Self::from_exprs(if_.cond, false),
                    then: Self::from_exprs(if_.then, is_tail),
                    els: Self::from_exprs(if_.els, is_tail),
                },
            )
            .with_span(expr.span),
            Expr::Call(_, call) => SelfExpr::Call(
                TailCallCallR { is_tail },
                Call {
                    func: Self::from_exprs(call.func, false),
                    args: call
                        .args
                        .into_iter()
                        .map(|arg| Self::from_exprs(arg, false))
                        .collect(),
                },
            )
            .with_span(expr.span),
            Expr::Begin(x, _) => x,
            Expr::Set(_, set) => SelfExpr::Set(
                (),
                Set {
                    name: set.name,
                    expr: Self::from_exprs(set.expr, false),
                },
            )
            .with_span(expr.span),
            Expr::Let(_, let_) => SelfExpr::Let(
                (),
                Let {
                    bindings: let_
                        .bindings
                        .into_iter()
                        .map(|binding| {
                            Binding {
                                name: binding.value.name,
                                expr: Self::from_exprs(binding.value.expr, false),
                            }
                            .with_span(binding.span)
                        })
                        .collect(),
                    body: Self::from_exprs(let_.body, is_tail),
                },
            )
            .with_span(expr.span),
            Expr::LetRec(_, letrec) => SelfExpr::LetRec(
                (),
                LetRec {
                    bindings: letrec
                        .bindings
                        .into_iter()
                        .map(|binding| {
                            Binding {
                                name: binding.value.name,
                                expr: Self::from_exprs(binding.value.expr, false),
                            }
                            .with_span(binding.span)
                        })
                        .collect(),
                    body: Self::from_exprs(letrec.body, is_tail),
                },
            )
            .with_span(expr.span),
            Expr::Vector(_, vec) => SelfExpr::Vector((), {
                vec.into_iter()
                    .map(|expr| Self::from_exprs(expr, false))
                    .collect()
            })
            .with_span(expr.span),
            Expr::UVector(_, uvec) => SelfExpr::UVector(
                (),
                UVector {
                    kind: uvec.kind,
                    elements: uvec
                        .elements
                        .into_iter()
                        .map(|expr| Self::from_exprs(expr, false))
                        .collect(),
                },
            )
            .with_span(expr.span),
            Expr::Quote(x, _) => x,
            Expr::Cons(_, cons) => SelfExpr::Cons(
                (),
                Cons {
                    car: Self::from_exprs(cons.car, false),
                    cdr: Self::from_exprs(cons.cdr, false),
                },
            )
            .with_span(expr.span),
        }
    }

    fn from_exprs<P: TailCallPrevPhase>(exprs: Vec<LExpr<P>>, is_tail: bool) -> Vec<Self> {
        let n = exprs.len();
        exprs
            .into_iter()
            .enumerate()
            .map(|(i, expr)| Self::from_expr(expr, is_tail && i == n - 1))
            .collect()
    }
}
