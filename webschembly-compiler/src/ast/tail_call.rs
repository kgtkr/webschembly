use super::astx::*;
use webschembly_compiler_locate::LocatedValue;

#[derive(Debug, Clone)]
pub enum TailCall {}

impl AstPhase for TailCall {
    type XBegin = !;
    type XQuote = !;
    type XDefine = !;
    type XCall = TailCallCallR;
}

#[derive(Debug, Clone)]
pub struct TailCallCallR {
    pub is_tail: bool,
}

pub trait TailCallPrevPhase = AstPhase<XBegin = !, XQuote = !, XDefine = !>;

impl TailCall {
    pub fn from_ast<P: TailCallPrevPhase>(ast: Ast<P>) -> Ast<Self> {
        Ast {
            x: (),
            exprs: ast
                .exprs
                .into_iter()
                .map(|expr| Self::from_expr(expr, false))
                .collect(),
        }
    }

    fn from_expr<P: TailCallPrevPhase>(expr: LExpr<P>, is_tail: bool) -> LExpr<Self> {
        match expr.value {
            Expr::Const(_, lit) => Expr::Const((), lit).with_span(expr.span),
            Expr::Var(_, var) => Expr::Var((), var).with_span(expr.span),
            Expr::Define(x, _) => x,
            Expr::Lambda(_, lambda) => Expr::Lambda(
                (),
                Lambda {
                    args: lambda.args,
                    body: Self::from_exprs(lambda.body, true),
                },
            )
            .with_span(expr.span),
            Expr::If(_, if_) => Expr::If(
                (),
                If {
                    cond: Self::from_exprs(if_.cond, false),
                    then: Self::from_exprs(if_.then, is_tail),
                    els: Self::from_exprs(if_.els, is_tail),
                },
            )
            .with_span(expr.span),
            Expr::Call(_, call) => Expr::Call(
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
            Expr::Set(_, set) => Expr::Set(
                (),
                Set {
                    name: set.name,
                    expr: Self::from_exprs(set.expr, false),
                },
            )
            .with_span(expr.span),
            Expr::Let(_, let_) => Expr::Let(
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
            Expr::LetRec(_, letrec) => Expr::LetRec(
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
            Expr::Vector(_, vec) => Expr::Vector((), {
                vec.into_iter()
                    .map(|expr| Self::from_exprs(expr, false))
                    .collect()
            })
            .with_span(expr.span),
            Expr::UVector(_, uvec) => Expr::UVector(
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
            Expr::Cons(_, cons) => Expr::Cons(
                (),
                Cons {
                    car: Self::from_exprs(cons.car, false),
                    cdr: Self::from_exprs(cons.cdr, false),
                },
            )
            .with_span(expr.span),
        }
    }

    fn from_exprs<P: TailCallPrevPhase>(exprs: Vec<LExpr<P>>, is_tail: bool) -> Vec<LExpr<Self>> {
        let n = exprs.len();
        exprs
            .into_iter()
            .enumerate()
            .map(|(i, expr)| Self::from_expr(expr, is_tail && i == n - 1))
            .collect()
    }
}
