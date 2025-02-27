use super::ast::*;
use crate::compiler_error;
use crate::error::Result;
use crate::sexpr::SExpr;
use crate::x::FamilyX;
#[derive(Debug, Clone)]
pub enum Parsed {}

impl FamilyX<Parsed> for AstX {
    type R = ();
}
impl FamilyX<Parsed> for LiteralX {
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

impl Ast<Parsed> {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Self> {
        let exprs = exprs
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(Ast { x: (), exprs })
    }
}

impl Expr<Parsed> {
    fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr::Bool(b) => Ok(Expr::Literal((), Literal::Bool(b))),
            SExpr::Int(i) => Ok(Expr::Literal((), Literal::Int(i))),
            SExpr::String(s) => Ok(Expr::Literal((), Literal::String(s))),
            SExpr::Symbol(s) => Ok(Expr::Var((), s)),
            SExpr::Nil => Ok(Expr::Literal((), Literal::Nil)),
            SExpr::Char(c) => Ok(Expr::Literal((), Literal::Char(c))),
            list_pattern![SExpr::Symbol("quote"), ..cdr] => match cdr {
                list_pattern![sexpr] => Ok(Expr::Literal((), Literal::Quote(sexpr))),
                _ => Err(compiler_error!("Invalid quote expression")),
            },
            list_pattern![SExpr::Symbol("define"), ..cdr] => match cdr {
                list_pattern![SExpr::Symbol(name), expr] => Ok(Expr::Define(
                    (),
                    Define {
                        name,
                        expr: Box::new(Expr::from_sexpr(expr)?),
                    },
                )),
                list_pattern![list_pattern![name, ..args], ..expr] => Expr::from_sexpr(list![
                    SExpr::Symbol("define".to_string()),
                    name,
                    list![SExpr::Symbol("lambda".to_string()), args, ..expr]
                ]),
                _ => Err(compiler_error!("Invalid define expression")),
            },
            list_pattern![SExpr::Symbol("lambda"), ..cdr] => match cdr {
                list_pattern![args, ..sexprs] => {
                    let args = args
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of symbols"))?;
                    let args = args
                        .into_iter()
                        .map(|arg| match arg {
                            SExpr::Symbol(s) => Ok(s),
                            _ => Err(compiler_error!("Expected a symbol")),
                        })
                        .collect::<Result<Vec<String>>>()?;
                    let sexprs = sexprs
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of expressions"))?;
                    let exprs = sexprs
                        .into_iter()
                        .map(Expr::from_sexpr)
                        .collect::<Result<Vec<_>>>()?;
                    Ok(Expr::Lambda((), Lambda { args, body: exprs }))
                }
                _ => Err(compiler_error!("Invalid lambda expression")),
            },
            list_pattern![SExpr::Symbol("if"), ..cdr] => match cdr {
                list_pattern![cond, then, els] => {
                    let cond = Expr::from_sexpr(cond)?;
                    let then = Expr::from_sexpr(then)?;
                    let els = Expr::from_sexpr(els)?;
                    Ok(Expr::If(
                        (),
                        If {
                            cond: Box::new(cond),
                            then: Box::new(then),
                            els: Box::new(els),
                        },
                    ))
                }
                _ => Err(compiler_error!("Invalid if expression",)),
            },
            list_pattern![SExpr::Symbol("let"), ..cdr] => match cdr {
                // TODO: 効率が悪いが一旦ラムダ式に変換
                list_pattern![bindings, ..body_sexprs] => {
                    let bindings = bindings
                        .to_vec()
                        .ok_or_else(|| compiler_error!("Expected a list of bindings"))?;
                    let bindings = bindings
                        .into_iter()
                        .map(|binding| match binding {
                            list_pattern![name, value] => Ok((name, value)),
                            _ => Err(compiler_error!("Invalid binding")),
                        })
                        .collect::<Result<Vec<(SExpr, SExpr)>>>()?;

                    let mut names = Vec::new();
                    let mut exprs = Vec::new();
                    for (name, value) in bindings {
                        names.push(name);
                        exprs.push(value);
                    }
                    let lambda = list![
                        SExpr::Symbol("lambda".to_string()),
                        SExpr::from_vec(names),
                        ..body_sexprs
                    ];

                    let exprs = SExpr::from_vec(exprs);

                    let result = list![lambda, ..exprs];
                    Expr::from_sexpr(result)
                }
                _ => Err(compiler_error!("Invalid let expression")),
            },
            list_pattern![SExpr::Symbol("begin"), ..cdr] => {
                let exprs = cdr
                    .to_vec()
                    .ok_or_else(|| compiler_error!("Invalid begin expression"))?;
                let exprs = exprs
                    .into_iter()
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Begin((), Begin { exprs }))
            }
            list_pattern![SExpr::Symbol("set!"), ..cdr] => match cdr {
                list_pattern![SExpr::Symbol(name), expr] => {
                    let expr = Expr::from_sexpr(expr)?;
                    Ok(Expr::Set(
                        (),
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
                    .ok_or_else(|| compiler_error!("Expected a list of arguments"))?;
                let args = args
                    .into_iter()
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call(
                    (),
                    Call {
                        func: Box::new(func),
                        args: args,
                    },
                ))
            }
        }
    }
}
