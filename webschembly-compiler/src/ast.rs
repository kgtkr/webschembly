use crate::sexpr::{Cons, SExpr};
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum AST {
    Bool(bool),
    Int(i32),
    String(String),
    Nil,
    Quote(SExpr),
    Define(String, Box<AST>),
    Lambda(Lambda),
    If(Box<AST>, Box<AST>, Box<AST>),
    Call(Box<AST>, Vec<AST>),
    Var(String),
    Begin(Vec<AST>),
    Dump(Box<AST>),
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub args: Vec<String>,
    pub body: Box<AST>,
}

impl AST {
    pub fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr::Bool(b) => Ok(AST::Bool(b)),
            SExpr::Int(i) => Ok(AST::Int(i)),
            SExpr::String(s) => Ok(AST::String(s)),
            SExpr::Symbol(s) => Ok(AST::Var(s)),
            SExpr::Nil => Ok(AST::Nil),
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("quote"),
                cdr,
            }) => match cdr {
                list_pattern![sexpr] => Ok(AST::Quote(sexpr)),
                _ => Err(anyhow::anyhow!("Invalid quote expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("define"),
                cdr,
            }) => match cdr {
                list_pattern![SExpr::Symbol(name), expr] => {
                    Ok(AST::Define(name, Box::new(AST::from_sexpr(expr)?)))
                }
                list_pattern![SExpr::Cons(box Cons { car, cdr }), expr] => AST::from_sexpr(list![
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
                list_pattern![args, expr] => {
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
                    let expr = AST::from_sexpr(expr)?;
                    Ok(AST::Lambda(Lambda {
                        args,
                        body: Box::new(expr),
                    }))
                }
                _ => Err(anyhow::anyhow!("Invalid lambda expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("if"),
                cdr,
            }) => match cdr {
                list_pattern![cond, then, els] => {
                    let cond = AST::from_sexpr(cond)?;
                    let then = AST::from_sexpr(then)?;
                    let els = AST::from_sexpr(els)?;
                    Ok(AST::If(Box::new(cond), Box::new(then), Box::new(els)))
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
                    AST::from_sexpr(SExpr::from_vec(result))
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
                    .map(AST::from_sexpr)
                    .collect::<Result<Vec<AST>>>()?;
                Ok(AST::Begin(exprs))
            }
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("dump"),
                cdr,
            }) => match cdr {
                list_pattern![expr] => Ok(AST::Dump(Box::new(AST::from_sexpr(expr)?))),
                _ => Err(anyhow::anyhow!("Invalid dump expression")),
            },
            SExpr::Cons(box Cons {
                car: func,
                cdr: args,
            }) => {
                let func = AST::from_sexpr(func)?;
                let args = args
                    .to_vec()
                    .ok_or_else(|| anyhow::anyhow!("Expected a list of arguments"))?;
                let args = args
                    .into_iter()
                    .map(AST::from_sexpr)
                    .collect::<Result<Vec<AST>>>()?;
                Ok(AST::Call(Box::new(func), args))
            }
        }
    }
}
