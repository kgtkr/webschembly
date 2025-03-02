use super::ast::*;
use crate::compiler_error;
use crate::error::Result;
use crate::sexpr::{SExpr, SExprKind};
use crate::span::Span;
use crate::x::{type_map, BasePhase, FamilyX, Phase};

#[derive(Debug, Clone)]
pub enum Parsed {}

impl Phase for Parsed {
    type Prev = BasePhase;
}

impl FamilyX<Parsed> for AstX {
    type R = ();
}

#[derive(Debug, Clone)]
pub struct ParsedLiteralR {
    pub span: Span,
}

impl FamilyX<Parsed> for LiteralX {
    type R = ParsedLiteralR;
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
    pub binding_spans: Vec<Span>,
}

impl FamilyX<Parsed> for LetX {
    type R = ParsedLetR;
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
            } => Ok(Expr::Literal(
                type_map::singleton(type_map::key::<Parsed>(), ParsedLiteralR { span }),
                Literal::Bool(b),
            )),
            SExpr {
                kind: SExprKind::Int(i),
                span,
                ..
            } => Ok(Expr::Literal(
                type_map::singleton(type_map::key::<Parsed>(), ParsedLiteralR { span }),
                Literal::Int(i),
            )),
            SExpr {
                kind: SExprKind::String(s),
                span,
                ..
            } => Ok(Expr::Literal(
                type_map::singleton(type_map::key::<Parsed>(), ParsedLiteralR { span }),
                Literal::String(s),
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
            } => Ok(Expr::Literal(
                type_map::singleton(type_map::key::<Parsed>(), ParsedLiteralR { span }),
                Literal::Nil,
            )),
            SExpr {
                kind: SExprKind::Char(c),
                span,
                ..
            } => Ok(Expr::Literal(
                type_map::singleton(type_map::key::<Parsed>(), ParsedLiteralR { span }),
                Literal::Char(c),
            )),
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("quote"),
                    span,
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![sexpr,] => Ok(Expr::Literal(
                    type_map::singleton(type_map::key::<Parsed>(), ParsedLiteralR { span }),
                    Literal::Quote(sexpr),
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
                        expr: Box::new(Expr::from_sexpr(expr)?),
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
                        expr: Box::new(Self::parse_lambda(lambda_span, args, exprs)?),
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
                            cond: Box::new(cond),
                            then: Box::new(then),
                            els: Box::new(els),
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
                // TODO: 効率が悪いが一旦ラムダ式に変換
                list_pattern![bindings, ..body] => {
                    let (bindings, binding_spans) = bindings
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of bindings"))?
                        .into_iter()
                        .map(|binding| match binding {
                            list_pattern![
                                SExpr {
                                    kind: SExprKind::String(name),
                                    ..
                                },
                                expr,
                            ] => Ok(((name, Expr::from_sexpr(expr)?), binding.span)),
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
                                binding_spans,
                            },
                        ),
                        Let { bindings, body },
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
                            expr: Box::new(expr),
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
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call(
                    type_map::singleton(type_map::key::<Parsed>(), ParsedCallR { span }),
                    Call {
                        func: Box::new(func),
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
