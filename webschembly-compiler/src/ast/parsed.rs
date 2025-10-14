use super::astx::*;
use crate::compiler_error;
use crate::error::Result;
use crate::sexpr::{SExpr, SExprKind};
use crate::span::Span;
use crate::x::{FamilyX, Phase};

#[derive(Debug, Clone)]
pub enum Parsed {}

impl Phase for Parsed {}

impl FamilyX<Parsed> for AstX {
    type R = ();
}

impl FamilyX<Parsed> for ConstX {
    type R = ();
}

impl FamilyX<Parsed> for DefineX {
    type R = ();
}

impl FamilyX<Parsed> for LambdaX {
    type R = ();
}

impl FamilyX<Parsed> for IfX {
    type R = ();
}

impl FamilyX<Parsed> for CallX {
    type R = ();
}

impl FamilyX<Parsed> for VarX {
    type R = ();
}

impl FamilyX<Parsed> for BeginX {
    type R = ();
}

impl FamilyX<Parsed> for SetX {
    type R = ();
}

impl FamilyX<Parsed> for LetX {
    type R = ();
}

impl FamilyX<Parsed> for LetRecX {
    type R = ();
}

impl FamilyX<Parsed> for VectorX {
    type R = ();
}

impl FamilyX<Parsed> for UVectorX {
    type R = ();
}

impl FamilyX<Parsed> for QuoteX {
    type R = ();
}

impl FamilyX<Parsed> for ConsX {
    type R = ();
}

impl Ast<Parsed> {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Self> {
        let exprs = exprs
            .into_iter()
            .map(LExpr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Ast { x: (), exprs })
    }
}

impl LExpr<Parsed> {
    fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr {
                kind: SExprKind::Bool(b),
                span,
                ..
            } => Ok(Expr::Const((), Const::Bool(b)).with_span(span)),
            SExpr {
                kind: SExprKind::Int(i),
                span,
                ..
            } => Ok(Expr::Const((), Const::Int(i)).with_span(span)),
            SExpr {
                kind: SExprKind::Float(f),
                span,
                ..
            } => Ok(Expr::Const((), Const::Float(f)).with_span(span)),
            SExpr {
                kind: SExprKind::NaN,
                span,
                ..
            } => Ok(Expr::Const((), Const::NaN).with_span(span)),
            SExpr {
                kind: SExprKind::String(s),
                span,
                ..
            } => Ok(Expr::Const((), Const::String(s)).with_span(span)),
            SExpr {
                kind: SExprKind::Symbol(s),
                span,
            } => Ok(Expr::Var((), s).with_span(span)),
            SExpr {
                kind: SExprKind::Nil,
                span,
                ..
            } => Ok(Expr::Const((), Const::Nil).with_span(span)),
            SExpr {
                kind: SExprKind::Char(c),
                span,
                ..
            } => Ok(Expr::Const((), Const::Char(c)).with_span(span)),
            sexpr @ SExpr {
                kind: SExprKind::Vector(_),
                span,
                ..
            } =>
            // #(...) は一旦 '#() として解釈して後で処理する
            // TODO: 少し汚い。unquoteなどを実装したときに問題が起きないか
            {
                Ok(Expr::Quote((), sexpr).with_span(span))
            }
            // TODO: uvectorも同様
            sexpr @ SExpr {
                kind: SExprKind::UVector(_, _),
                span,
                ..
            } => Ok(Expr::Quote((), sexpr).with_span(span)),
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("quote"),
                    span,
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![sexpr,] => Ok(Expr::Quote((), sexpr).with_span(span)),
                _ => Err(compiler_error!("Invalid quote expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("define"),
                    ..
                } => span,
                ..cdr
            ] => match cdr {
                list_pattern![
                    SExpr {
                        kind: SExprKind::Symbol(name),
                        span: name_span
                    },
                    expr,
                ] => Ok(Expr::Define(
                    (),
                    Define {
                        name: name.with_span(name_span),
                        expr: vec![Self::from_sexpr(expr)?],
                    },
                )
                .with_span(span)),
                list_pattern![
                    list_pattern![
                        SExpr {
                            kind: SExprKind::Symbol(name),
                            span: name_span
                        },
                        ..args
                    ] => lambda_span,
                    ..exprs
                ] => Ok(Expr::Define(
                    (),
                    Define {
                        name: name.with_span(name_span),
                        expr: vec![Self::parse_lambda(lambda_span, args, exprs)?],
                    },
                )
                .with_span(span)),
                _ => Err(compiler_error!("Invalid define expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("lambda"),
                    ..
                } => span,
                ..cdr
            ] => match cdr {
                list_pattern![args, ..exprs] => Self::parse_lambda(span, args, exprs),
                _ => Err(compiler_error!("Invalid lambda expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("if"),
                    ..
                } => span,
                ..cdr
            ] => match cdr {
                list_pattern![cond, then, els,] => {
                    let cond = Self::from_sexpr(cond)?;
                    let then = Self::from_sexpr(then)?;
                    let els = Self::from_sexpr(els)?;
                    Ok(Expr::If(
                        (),
                        If {
                            cond: vec![cond],
                            then: vec![then],
                            els: vec![els],
                        },
                    )
                    .with_span(span))
                }
                _ => Err(compiler_error!("Invalid if expression",)),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("let"),
                    ..
                } => span,
                ..cdr
            ] => match cdr {
                list_pattern![bindings, ..body] => {
                    let bindings = bindings
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of bindings"))?
                        .into_iter()
                        .map(|binding| match binding {
                            list_pattern![
                                SExpr {
                                    kind: SExprKind::Symbol(name),
                                    ..
                                } => name_span,
                                expr,
                            ] => Ok(Binding {
                                name: name.with_span(name_span),
                                expr: vec![Self::from_sexpr(expr)?],
                            }
                            .with_span(binding.span)),
                            _ => Err(compiler_error!("Invalid binding")),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let body = body
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
                        .into_iter()
                        .map(Self::from_sexpr)
                        .collect::<Result<Vec<_>>>()?;

                    Ok(Expr::Let((), Let { bindings, body }).with_span(span))
                }
                _ => Err(compiler_error!("Invalid let expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("letrec"),
                    ..
                } => span,
                ..cdr
            ] => match cdr {
                list_pattern![bindings, ..body] => {
                    let bindings = bindings
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of bindings"))?
                        .into_iter()
                        .map(|binding| match binding {
                            list_pattern![
                                SExpr {
                                    kind: SExprKind::Symbol(name),
                                    ..
                                } => name_span,
                                expr,
                            ] => Ok(Binding {
                                name: name.with_span(name_span),
                                expr: vec![Self::from_sexpr(expr)?],
                            }
                            .with_span(binding.span)),
                            _ => Err(compiler_error!("Invalid binding")),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let body = body
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
                        .into_iter()
                        .map(Self::from_sexpr)
                        .collect::<Result<Vec<_>>>()?;

                    Ok(Expr::LetRec((), LetRec { bindings, body }).with_span(span))
                }
                _ => Err(compiler_error!("Invalid let expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("begin"),
                    ..
                } => span,
                ..exprs
            ] => {
                let exprs = exprs
                    .to_vec()
                    .ok_or_else(|| compiler_error!("Invalid begin expression"))?
                    .into_iter()
                    .map(Self::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Begin((), Begin { exprs }).with_span(span))
            }
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("set!"),
                    ..
                } => span,
                ..cdr
            ] => match cdr {
                list_pattern![
                    SExpr {
                        kind: SExprKind::Symbol(name),
                        span: name_span
                    },
                    expr,
                ] => {
                    let expr = Self::from_sexpr(expr)?;
                    Ok(Expr::Set(
                        (),
                        Set {
                            name: name.with_span(name_span),
                            expr: vec![expr],
                        },
                    )
                    .with_span(span))
                }
                _ => Err(compiler_error!("Invalid set! expression")),
            },
            list_pattern![func => span, ..args] => {
                let func = Self::from_sexpr(func)?;
                let args = args
                    .to_vec()
                    .ok_or_else(|| compiler_error!("Expected a list of arguments"))?
                    .into_iter()
                    .map(|expr| Self::from_sexpr(expr).map(|e| vec![e]))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call(
                    (),
                    Call {
                        func: vec![func],
                        args,
                    },
                )
                .with_span(span))
            }
        }
    }

    fn parse_lambda(span: Span, args: SExpr, exprs: SExpr) -> Result<Self> {
        let args = args
            .to_vec()
            .ok_or_else(|| compiler_error!("Expected a list of symbols"))?
            .into_iter()
            .map(|arg| match arg.kind {
                SExprKind::Symbol(s) => Ok(s.with_span(arg.span)),
                _ => Err(compiler_error!("Expected a symbol")),
            })
            .collect::<Result<Vec<_>>>()?;
        let exprs = exprs
            .to_vec()
            .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
            .into_iter()
            .map(Self::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Expr::Lambda((), Lambda { args, body: exprs }).with_span(span))
    }
}
