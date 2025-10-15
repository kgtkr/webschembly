use webschembly_compiler_locate::LocatedValue;

use super::astx::*;
use crate::sexpr;

#[derive(Debug, Clone)]
pub enum Desugared {}

impl AstPhase for Desugared {
    type XBegin = !;
    type XQuote = !;
}

pub trait DesugaredPrevPhase = AstPhase;

impl Desugared {
    pub fn from_ast<P: DesugaredPrevPhase>(ast: Ast<P>) -> Ast<Self> {
        Ast {
            x: (),
            exprs: Self::from_exprs(ast.exprs),
        }
    }

    fn from_expr<P: DesugaredPrevPhase>(expr: LExpr<P>, exprs: &mut Vec<LExpr<Self>>) {
        match expr.value {
            Expr::Const(_, lit) => exprs.push(Expr::Const((), lit).with_span(expr.span)),
            Expr::Var(_, var) => exprs.push(Expr::Var((), var).with_span(expr.span)),
            Expr::Define(_, def) => exprs.push(
                Expr::Define(
                    (),
                    Define {
                        name: def.name,
                        expr: Self::from_exprs(def.expr),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Lambda(_, lambda) => exprs.push(
                Expr::Lambda(
                    (),
                    Lambda {
                        args: lambda.args,
                        body: Self::from_exprs(lambda.body),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::If(_, if_) => exprs.push(
                Expr::If(
                    (),
                    If {
                        cond: Self::from_exprs(if_.cond),
                        then: Self::from_exprs(if_.then),
                        els: Self::from_exprs(if_.els),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Call(_, call) => exprs.push(
                Expr::Call(
                    (),
                    Call {
                        func: Self::from_exprs(call.func),
                        args: call.args.into_iter().map(Self::from_exprs).collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Begin(_, begin) => {
                for expr in begin.exprs {
                    Self::from_expr(expr, exprs);
                }
            }
            Expr::Set(_, set) => exprs.push(
                Expr::Set(
                    (),
                    Set {
                        name: set.name,
                        expr: Self::from_exprs(set.expr),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Let(_, let_) => exprs.push(
                Expr::Let(
                    (),
                    Let {
                        bindings: let_
                            .bindings
                            .into_iter()
                            .map(|binding| {
                                Binding {
                                    name: binding.value.name,
                                    expr: Self::from_exprs(binding.value.expr),
                                }
                                .with_span(binding.span)
                            })
                            .collect(),
                        body: Self::from_exprs(let_.body),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::LetRec(_, letrec) => exprs.push(
                Expr::LetRec(
                    (),
                    LetRec {
                        bindings: letrec
                            .bindings
                            .into_iter()
                            .map(|binding| {
                                Binding {
                                    name: binding.value.name,
                                    expr: Self::from_exprs(binding.value.expr),
                                }
                                .with_span(binding.span)
                            })
                            .collect(),
                        body: Self::from_exprs(letrec.body),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Vector(_, vec) => exprs.push(
                Expr::Vector((), vec.into_iter().map(Self::from_exprs).collect())
                    .with_span(expr.span),
            ),
            Expr::UVector(_, uvec) => exprs.push(
                Expr::UVector(
                    (),
                    UVector {
                        kind: uvec.kind,
                        elements: uvec.elements.into_iter().map(Self::from_exprs).collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Quote(_, sexpr) => exprs.push(Self::from_quoted_sexpr(sexpr)),
            Expr::Cons(_, cons) => exprs.push(
                Expr::Cons(
                    (),
                    Cons {
                        car: Self::from_exprs(cons.car),
                        cdr: Self::from_exprs(cons.cdr),
                    },
                )
                .with_span(expr.span),
            ),
        }
    }

    fn from_quoted_sexpr(sexpr: sexpr::SExpr) -> LExpr<Self> {
        match sexpr.kind {
            sexpr::SExprKind::Bool(b) => Expr::Const((), Const::Bool(b)).with_span(sexpr.span),
            sexpr::SExprKind::Int(i) => Expr::Const((), Const::Int(i)).with_span(sexpr.span),
            sexpr::SExprKind::Float(f) => Expr::Const((), Const::Float(f)).with_span(sexpr.span),
            sexpr::SExprKind::NaN => Expr::Const((), Const::NaN).with_span(sexpr.span),
            sexpr::SExprKind::String(s) => Expr::Const((), Const::String(s)).with_span(sexpr.span),
            sexpr::SExprKind::Char(c) => Expr::Const((), Const::Char(c)).with_span(sexpr.span),
            sexpr::SExprKind::Symbol(s) => Expr::Const((), Const::Symbol(s)).with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExprKind::Cons(cons) => Expr::Cons(
                (),
                Cons {
                    car: vec![Self::from_quoted_sexpr(cons.car)],
                    cdr: vec![Self::from_quoted_sexpr(cons.cdr)],
                },
            )
            .with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExprKind::Vector(vec) => Expr::Vector(
                (),
                vec.into_iter()
                    .map(|s| vec![Self::from_quoted_sexpr(s)])
                    .collect(),
            )
            .with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExprKind::UVector(kind, elements) => Expr::UVector(
                (),
                UVector {
                    kind: match kind {
                        sexpr::SUVectorKind::S64 => UVectorKind::S64,
                        sexpr::SUVectorKind::F64 => UVectorKind::F64,
                    },
                    elements: elements
                        .into_iter()
                        .map(|s| vec![Self::from_quoted_sexpr(s)])
                        .collect(),
                },
            )
            .with_span(sexpr.span),
            sexpr::SExprKind::Nil => Expr::Const((), Const::Nil).with_span(sexpr.span),
        }
    }

    fn from_exprs<P: DesugaredPrevPhase>(exprs: Vec<LExpr<P>>) -> Vec<LExpr<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            Self::from_expr(expr, &mut result);
        }
        result
    }
}
