use super::astx::*;
use webschembly_compiler_error::{Result, compiler_error};
use webschembly_compiler_locate::{LocatedValue, Span};
use webschembly_compiler_sexpr::{SExpr, SExprKind, list_pattern};

#[derive(Debug, Clone)]
pub struct Parsed;

impl AstPhase for Parsed {
    type XAst = ();
    type XConst = ();
    type XDefine = ();
    type XLambda = ();
    type XIf = ();
    type XCall = ();
    type XVar = ();
    type XBegin = ();
    type XSet = ();
    type XLet = ();
    type XLetRec = ();
    type XVector = ();
    type XUVector = ();
    type XQuote = ();
    type XCons = ();
}

impl Parsed {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Ast<Self>> {
        let exprs = exprs
            .into_iter()
            .map(Self::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Ast { x: (), exprs })
    }

    fn from_sexpr(sexpr: SExpr) -> Result<LExpr<Self>> {
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

    fn parse_lambda(span: Span, args: SExpr, exprs: SExpr) -> Result<LExpr<Self>> {
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
