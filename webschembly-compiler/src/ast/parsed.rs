use frunk::hlist::h_cons;
use frunk::{field, HNil};

use super::ast::*;
use crate::error::Result;
use crate::sexpr::{SExpr, SExprKind};
use crate::x::{BasePhase, FamilyX, Phase};
use crate::{compiler_error, family_x_rs};
#[derive(Debug, Clone)]
pub enum Parsed {}

impl Phase for Parsed {
    type Prev = BasePhase;
}

impl FamilyX<Parsed> for AstX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for LiteralX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for DefineX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for LambdaX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for IfX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for CallX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for VarX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for BeginX {
    type R = ();
    type RS = family_x_rs!();
}
impl FamilyX<Parsed> for SetX {
    type R = ();
    type RS = family_x_rs!();
}

impl FamilyX<Parsed> for LetX {
    type R = ();
    type RS = family_x_rs!();
}

impl Ast<Parsed> {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Self> {
        let exprs = exprs
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Ast {
            x: h_cons(field![Parsed, ()], HNil),
            exprs,
        })
    }
}

impl Expr<Parsed> {
    fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr {
                kind: SExprKind::Bool(b),
                ..
            } => Ok(Expr::Literal(
                h_cons(field![Parsed, ()], HNil),
                Literal::Bool(b),
            )),
            SExpr {
                kind: SExprKind::Int(i),
                ..
            } => Ok(Expr::Literal(
                h_cons(field![Parsed, ()], HNil),
                Literal::Int(i),
            )),
            SExpr {
                kind: SExprKind::String(s),
                ..
            } => Ok(Expr::Literal(
                h_cons(field![Parsed, ()], HNil),
                Literal::String(s),
            )),
            SExpr {
                kind: SExprKind::Symbol(s),
                ..
            } => Ok(Expr::Var(h_cons(field![Parsed, ()], HNil), s)),
            SExpr {
                kind: SExprKind::Nil,
                ..
            } => Ok(Expr::Literal(
                h_cons(field![Parsed, ()], HNil),
                Literal::Nil,
            )),
            SExpr {
                kind: SExprKind::Char(c),
                ..
            } => Ok(Expr::Literal(
                h_cons(field![Parsed, ()], HNil),
                Literal::Char(c),
            )),
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("quote"),
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![sexpr,] => Ok(Expr::Literal(
                    h_cons(field![Parsed, ()], HNil),
                    Literal::Quote(sexpr),
                )),
                _ => Err(compiler_error!("Invalid quote expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("define"),
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![
                    SExpr {
                        kind: SExprKind::Symbol(name),
                        ..
                    },
                    expr,
                ] => Ok(Expr::Define(
                    h_cons(field![Parsed, ()], HNil),
                    Define {
                        name,
                        expr: Box::new(Expr::from_sexpr(expr)?),
                    },
                )),
                list_pattern![
                    list_pattern![
                        SExpr {
                            kind: SExprKind::Symbol(name),
                            ..
                        },
                        ..args
                    ],
                    ..exprs
                ] => Ok(Expr::Define(
                    h_cons(field![Parsed, ()], HNil),
                    Define {
                        name,
                        expr: Box::new(Self::parse_lambda(args, exprs)?),
                    },
                )),
                _ => Err(compiler_error!("Invalid define expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("lambda"),
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![args, ..exprs] => Self::parse_lambda(args, exprs),
                _ => Err(compiler_error!("Invalid lambda expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("if"),
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![cond, then, els,] => {
                    let cond = Expr::from_sexpr(cond)?;
                    let then = Expr::from_sexpr(then)?;
                    let els = Expr::from_sexpr(els)?;
                    Ok(Expr::If(
                        h_cons(field![Parsed, ()], HNil),
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
                },
                ..cdr
            ] => match cdr {
                // TODO: 効率が悪いが一旦ラムダ式に変換
                list_pattern![bindings, ..body] => {
                    let bindings = bindings
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
                            ] => Ok((name, Expr::from_sexpr(expr)?)),
                            _ => Err(compiler_error!("Invalid binding")),
                        })
                        .collect::<Result<Vec<(_, _)>>>()?;
                    let body = body
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
                        .into_iter()
                        .map(Expr::from_sexpr)
                        .collect::<Result<Vec<_>>>()?;

                    Ok(Expr::Let(
                        h_cons(field![Parsed, ()], HNil),
                        Let { bindings, body },
                    ))
                }
                _ => Err(compiler_error!("Invalid let expression")),
            },
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("begin"),
                    ..
                },
                ..exprs
            ] => {
                let exprs = exprs
                    .to_vec()
                    .ok_or_else(|| compiler_error!("Invalid begin expression"))?
                    .into_iter()
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Begin(
                    h_cons(field![Parsed, ()], HNil),
                    Begin { exprs },
                ))
            }
            list_pattern![
                SExpr {
                    kind: SExprKind::Symbol("set!"),
                    ..
                },
                ..cdr
            ] => match cdr {
                list_pattern![
                    SExpr {
                        kind: SExprKind::Symbol(name),
                        ..
                    },
                    expr,
                ] => {
                    let expr = Expr::from_sexpr(expr)?;
                    Ok(Expr::Set(
                        h_cons(field![Parsed, ()], HNil),
                        Set {
                            name,
                            expr: Box::new(expr),
                        },
                    ))
                }
                _ => Err(compiler_error!("Invalid set! expression")),
            },
            list_pattern![func, ..args] => {
                let func = Expr::from_sexpr(func)?;
                let args = args
                    .to_vec()
                    .ok_or_else(|| compiler_error!("Expected a list of arguments"))?
                    .into_iter()
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call(
                    h_cons(field![Parsed, ()], HNil),
                    Call {
                        func: Box::new(func),
                        args: args,
                    },
                ))
            }
        }
    }

    fn parse_lambda(args: SExpr, exprs: SExpr) -> Result<Self> {
        let args = args
            .to_vec()
            .ok_or_else(|| compiler_error!("Expected a list of symbols"))?
            .into_iter()
            .map(|arg| match arg.kind {
                SExprKind::Symbol(s) => Ok(s),
                _ => Err(compiler_error!("Expected a symbol")),
            })
            .collect::<Result<Vec<String>>>()?;
        let exprs = exprs
            .to_vec()
            .ok_or_else(|| compiler_error!("Expected a list of expressions"))?
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Expr::Lambda(
            h_cons(field![Parsed, ()], HNil),
            Lambda { args, body: exprs },
        ))
    }
}
