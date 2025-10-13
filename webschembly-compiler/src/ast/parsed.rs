use super::astx::*;
use crate::compiler_error;
use crate::error::Result;
use crate::sexpr::{SExpr, SExprKind};
use crate::span::Span;
use crate::x::{BasePhase, FamilyX, Phase, type_map};

#[derive(Debug, Clone)]
pub enum Parsed {}

impl Phase for Parsed {
    type Prev = BasePhase;
}

impl FamilyX<Parsed> for AstX {
    type R = ();
}

#[derive(Debug, Clone)]
pub struct ParsedConstR {
    pub span: Span,
}

impl FamilyX<Parsed> for ConstX {
    type R = ParsedConstR;
}

#[derive(Debug, Clone)]
pub struct ParsedDefineR {
    pub span: Span,
    pub name_span: Span,
}

impl FamilyX<Parsed> for DefineX {
    type R = ParsedDefineR;
}

#[derive(Debug, Clone)]
pub struct ParsedLambdaR {
    pub span: Span,
    pub arg_spans: Vec<Span>,
}

impl FamilyX<Parsed> for LambdaX {
    type R = ParsedLambdaR;
}

#[derive(Debug, Clone)]
pub struct ParsedIfR {
    pub span: Span,
}

impl FamilyX<Parsed> for IfX {
    type R = ParsedIfR;
}

#[derive(Debug, Clone)]
pub struct ParsedCallR {
    pub span: Span,
}

impl FamilyX<Parsed> for CallX {
    type R = ParsedCallR;
}

#[derive(Debug, Clone)]
pub struct ParsedVarR {
    pub span: Span,
}

impl FamilyX<Parsed> for VarX {
    type R = ParsedVarR;
}

#[derive(Debug, Clone)]
pub struct ParsedBeginR {
    pub span: Span,
}

impl FamilyX<Parsed> for BeginX {
    type R = ParsedBeginR;
}

#[derive(Debug, Clone)]
pub struct ParsedSetR {
    pub span: Span,
    pub name_span: Span,
}

impl FamilyX<Parsed> for SetX {
    type R = ParsedSetR;
}

#[derive(Debug, Clone)]
pub struct ParsedLetR {
    pub span: Span,
    pub binding_name_spans: Vec<Span>,
}

impl FamilyX<Parsed> for LetX {
    type R = ParsedLetR;
}

#[derive(Debug, Clone)]
pub struct ParsedLetRecR {
    pub span: Span,
    pub binding_name_spans: Vec<Span>,
}

impl FamilyX<Parsed> for LetRecX {
    type R = ParsedLetRecR;
}

#[derive(Debug, Clone)]
pub struct ParsedVectorR {
    pub span: Span,
}
impl FamilyX<Parsed> for VectorX {
    type R = ParsedVectorR;
}

#[derive(Debug, Clone)]
pub struct ParsedUVectorR {
    pub span: Span,
}

impl FamilyX<Parsed> for UVectorX {
    type R = ParsedUVectorR;
}

#[derive(Debug, Clone)]
pub struct ParsedQuoteR {
    pub span: Span,
}
impl FamilyX<Parsed> for QuoteX {
    type R = ParsedQuoteR;
}

#[derive(Debug, Clone)]
pub struct ParsedConsR {
    pub span: Span,
}
impl FamilyX<Parsed> for ConsX {
    type R = ParsedConsR;
}

impl Ast<Parsed> {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Self> {
        let exprs = exprs
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Ast {
            x: type_map::singleton(type_map::key::<Parsed>(), ()),
            exprs,
        })
    }
}

impl Expr<Parsed> {
    fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr {
                kind: SExprKind::Bool(b),
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::Bool(b),
            )),
            SExpr {
                kind: SExprKind::Int(i),
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::Int(i),
            )),
            SExpr {
                kind: SExprKind::Float(f),
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::Float(f),
            )),
            SExpr {
                kind: SExprKind::NaN,
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::NaN,
            )),
            SExpr {
                kind: SExprKind::String(s),
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::String(s),
            )),
            SExpr {
                kind: SExprKind::Symbol(s),
                span,
            } => Ok(Expr::Var(
                type_map::singleton(type_map::key::<Parsed>(), ParsedVarR { span }),
                s,
            )),
            SExpr {
                kind: SExprKind::Nil,
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::Nil,
            )),
            SExpr {
                kind: SExprKind::Char(c),
                span,
                ..
            } => Ok(Expr::Const(
                type_map::singleton(type_map::key::<Parsed>(), ParsedConstR { span }),
                Const::Char(c),
            )),
            sexpr @ SExpr {
                kind: SExprKind::Vector(_),
                span,
                ..
            } =>
            // #(...) は一旦 '#() として解釈して後で処理する
            // TODO: 少し汚い。unquoteなどを実装したときに問題が起きないか
            {
                Ok(Expr::Quote(
                    type_map::singleton(type_map::key::<Parsed>(), ParsedQuoteR { span }),
                    sexpr,
                ))
            }
            // TODO: uvectorも同様
            sexpr @ SExpr {
                kind: SExprKind::UVector(_, _),
                span,
                ..
            } => Ok(Expr::Quote(
                type_map::singleton(type_map::key::<Parsed>(), ParsedQuoteR { span }),
                sexpr,
            )),
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("quote"),
                    span,
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![sexpr,] => Ok(Expr::Quote(
                    type_map::singleton(type_map::key::<Parsed>(), ParsedQuoteR { span }),
                    sexpr,
                )),
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
                    type_map::singleton(
                        type_map::key::<Parsed>(),
                        ParsedDefineR { span, name_span },
                    ),
                    Define {
                        name,
                        expr: vec![Expr::from_sexpr(expr)?],
                    },
                )),
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
                    type_map::singleton(
                        type_map::key::<Parsed>(),
                        ParsedDefineR { span, name_span },
                    ),
                    Define {
                        name,
                        expr: vec![Self::parse_lambda(lambda_span, args, exprs)?],
                    },
                )),
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
                    let cond = Expr::from_sexpr(cond)?;
                    let then = Expr::from_sexpr(then)?;
                    let els = Expr::from_sexpr(els)?;
                    Ok(Expr::If(
                        type_map::singleton(type_map::key::<Parsed>(), ParsedIfR { span }),
                        If {
                            cond: vec![cond],
                            then: vec![then],
                            els: vec![els],
                        },
                    ))
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
                    let (bindings, binding_name_spans) = bindings
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
                            ] => Ok(((name, vec![Expr::from_sexpr(expr)?]), name_span)),
                            _ => Err(compiler_error!("Invalid binding")),
                        })
                        .collect::<Result<(Vec<_>, Vec<_>)>>()?;
                    let body = body
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
                        .into_iter()
                        .map(Expr::from_sexpr)
                        .collect::<Result<Vec<_>>>()?;

                    Ok(Expr::Let(
                        type_map::singleton(
                            type_map::key::<Parsed>(),
                            ParsedLetR {
                                span,
                                binding_name_spans,
                            },
                        ),
                        Let { bindings, body },
                    ))
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
                    let (bindings, binding_name_spans) = bindings
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
                            ] => Ok(((name, vec![Expr::from_sexpr(expr)?]), name_span)),
                            _ => Err(compiler_error!("Invalid binding")),
                        })
                        .collect::<Result<(Vec<_>, Vec<_>)>>()?;
                    let body = body
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
                        .into_iter()
                        .map(Expr::from_sexpr)
                        .collect::<Result<Vec<_>>>()?;

                    Ok(Expr::LetRec(
                        type_map::singleton(
                            type_map::key::<Parsed>(),
                            ParsedLetRecR {
                                span,
                                binding_name_spans,
                            },
                        ),
                        LetRec { bindings, body },
                    ))
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
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Begin(
                    type_map::singleton(type_map::key::<Parsed>(), ParsedBeginR { span }),
                    Begin { exprs },
                ))
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
                    let expr = Expr::from_sexpr(expr)?;
                    Ok(Expr::Set(
                        type_map::singleton(
                            type_map::key::<Parsed>(),
                            ParsedSetR { span, name_span },
                        ),
                        Set {
                            name,
                            expr: vec![expr],
                        },
                    ))
                }
                _ => Err(compiler_error!("Invalid set! expression")),
            },
            list_pattern![func => span, ..args] => {
                let func = Expr::from_sexpr(func)?;
                let args = args
                    .to_vec()
                    .ok_or_else(|| compiler_error!("Expected a list of arguments"))?
                    .into_iter()
                    .map(|expr| Expr::from_sexpr(expr).map(|e| vec![e]))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call(
                    type_map::singleton(type_map::key::<Parsed>(), ParsedCallR { span }),
                    Call {
                        func: vec![func],
                        args,
                    },
                ))
            }
        }
    }

    fn parse_lambda(span: Span, args: SExpr, exprs: SExpr) -> Result<Self> {
        let (args, arg_spans) = args
            .to_vec()
            .ok_or_else(|| compiler_error!("Expected a list of symbols"))?
            .into_iter()
            .map(|arg| match arg.kind {
                SExprKind::Symbol(s) => Ok((s, arg.span)),
                _ => Err(compiler_error!("Expected a symbol")),
            })
            .collect::<Result<(Vec<_>, Vec<_>)>>()?;
        let exprs = exprs
            .to_vec()
            .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Expr::Lambda(
            type_map::singleton(type_map::key::<Parsed>(), ParsedLambdaR { span, arg_spans }),
            Lambda { args, body: exprs },
        ))
    }
}
