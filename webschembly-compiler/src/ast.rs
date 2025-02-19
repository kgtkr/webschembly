use crate::sexpr::{Cons, SExpr};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct AST {
    pub exprs: Vec<Expr>,
}

impl AST {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Self> {
        let exprs = exprs
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(AST { exprs })
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i32),
    String(String),
    Nil,
    Quote(SExpr),
    Define(String, Box<Expr>),
    Lambda(Lambda),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Var(String),
    Begin(Vec<Expr>),
    Dump(Box<Expr>),
}

impl Expr {
    fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr::Bool(b) => Ok(Expr::Bool(b)),
            SExpr::Int(i) => Ok(Expr::Int(i)),
            SExpr::String(s) => Ok(Expr::String(s)),
            SExpr::Symbol(s) => Ok(Expr::Var(s)),
            SExpr::Nil => Ok(Expr::Nil),
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("quote"),
                cdr,
            }) => match cdr {
                list_pattern![sexpr] => Ok(Expr::Quote(sexpr)),
                _ => Err(anyhow::anyhow!("Invalid quote expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("define"),
                cdr,
            }) => match cdr {
                list_pattern![SExpr::Symbol(name), expr] => {
                    Ok(Expr::Define(name, Box::new(Expr::from_sexpr(expr)?)))
                }
                list_pattern![SExpr::Cons(box Cons { car, cdr }), expr] => Expr::from_sexpr(list![
                    SExpr::Symbol("define".to_string()),
                    car,
                    list![SExpr::Symbol("lambda".to_string()), cdr, expr]
                ]),
                _ => Err(anyhow::anyhow!("Invalid define expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("lambda"),
                cdr,
            }) => match cdr {
                SExpr::Cons(box Cons {
                    car: args,
                    cdr: sexprs,
                }) => {
                    let args = args
                        .to_vec()
                        .ok_or_else(|| anyhow::anyhow!("Expected a list of symbols"))?;
                    let args = args
                        .into_iter()
                        .map(|arg| match arg {
                            SExpr::Symbol(s) => Ok(s),
                            _ => Err(anyhow::anyhow!("Expected a symbol")),
                        })
                        .collect::<Result<Vec<String>>>()?;
                    let sexprs = sexprs
                        .to_vec()
                        .ok_or_else(|| anyhow::anyhow!("Expected a list of expressions"))?;
                    let exprs = sexprs
                        .into_iter()
                        .map(Expr::from_sexpr)
                        .collect::<Result<Vec<Expr>>>()?;
                    Ok(Expr::Lambda(Lambda { args, body: exprs }))
                }
                _ => Err(anyhow::anyhow!("Invalid lambda expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("if"),
                cdr,
            }) => match cdr {
                list_pattern![cond, then, els] => {
                    let cond = Expr::from_sexpr(cond)?;
                    let then = Expr::from_sexpr(then)?;
                    let els = Expr::from_sexpr(els)?;
                    Ok(Expr::If(Box::new(cond), Box::new(then), Box::new(els)))
                }
                _ => Err(anyhow::anyhow!("Invalid if expression",)),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("let"),
                cdr,
            }) => match cdr {
                list_pattern![bindings, expr] => {
                    let bindings = bindings
                        .to_vec()
                        .ok_or_else(|| anyhow::anyhow!("Expected a list of bindings"))?;
                    let bindings = bindings
                        .into_iter()
                        .map(|binding| match binding {
                            list_pattern![name, value] => Ok((name, value)),
                            _ => Err(anyhow::anyhow!("Invalid binding")),
                        })
                        .collect::<Result<Vec<(SExpr, SExpr)>>>()?;

                    let mut names = Vec::new();
                    let mut exprs = Vec::new();
                    for (name, value) in bindings {
                        names.push(name);
                        exprs.push(value);
                    }

                    let mut result = vec![list![
                        SExpr::Symbol("lambda".to_string()),
                        SExpr::from_vec(names),
                        expr
                    ]];
                    result.extend(exprs);
                    Expr::from_sexpr(SExpr::from_vec(result))
                }
                _ => Err(anyhow::anyhow!("Invalid let expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("begin"),
                cdr,
            }) => {
                let exprs = cdr
                    .to_vec()
                    .ok_or_else(|| anyhow::anyhow!("Invalid begin expression"))?;
                let exprs = exprs
                    .into_iter()
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<Expr>>>()?;
                Ok(Expr::Begin(exprs))
            }
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("dump"),
                cdr,
            }) => match cdr {
                list_pattern![expr] => Ok(Expr::Dump(Box::new(Expr::from_sexpr(expr)?))),
                _ => Err(anyhow::anyhow!("Invalid dump expression")),
            },
            SExpr::Cons(box Cons {
                car: func,
                cdr: args,
            }) => {
                let func = Expr::from_sexpr(func)?;
                let args = args
                    .to_vec()
                    .ok_or_else(|| anyhow::anyhow!("Expected a list of arguments"))?;
                let args = args
                    .into_iter()
                    .map(Expr::from_sexpr)
                    .collect::<Result<Vec<Expr>>>()?;
                Ok(Expr::Call(Box::new(func), args))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub args: Vec<String>,
    pub body: Vec<Expr>,
}
