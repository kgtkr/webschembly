use crate::sexpr::{Cons, SExpr};
use crate::x::{FamilyX, RunX};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct AST<X>
where
    X: XBound,
{
    pub x: RunX<AstX, X>,
    pub exprs: Vec<Expr<X>>,
}

impl AST<Parsed> {
    pub fn from_sexprs(exprs: Vec<SExpr>) -> Result<Self> {
        let exprs = exprs
            .into_iter()
            .map(Expr::from_sexpr)
            .collect::<Result<Vec<_>>>()?;
        Ok(AST { x: (), exprs })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct AstX;
#[derive(Debug, Clone, Copy)]
pub struct BoolX;
#[derive(Debug, Clone, Copy)]
pub struct IntX;
#[derive(Debug, Clone, Copy)]
pub struct StringX;
#[derive(Debug, Clone, Copy)]
pub struct NilX;
#[derive(Debug, Clone, Copy)]
pub struct QuoteX;
#[derive(Debug, Clone, Copy)]

pub struct DefineX;
#[derive(Debug, Clone, Copy)]

pub struct LambdaX;
#[derive(Debug, Clone, Copy)]

pub struct IfX;
#[derive(Debug, Clone, Copy)]

pub struct CallX;
#[derive(Debug, Clone, Copy)]

pub struct VarX;
#[derive(Debug, Clone, Copy)]

pub struct BeginX;
#[derive(Debug, Clone, Copy)]

pub struct DumpX;

pub trait XBound = Sized
where
    AstX: FamilyX<Self>,
    BoolX: FamilyX<Self>,
    IntX: FamilyX<Self>,
    StringX: FamilyX<Self>,
    NilX: FamilyX<Self>,
    QuoteX: FamilyX<Self>,
    DefineX: FamilyX<Self>,
    LambdaX: FamilyX<Self>,
    IfX: FamilyX<Self>,
    CallX: FamilyX<Self>,
    VarX: FamilyX<Self>,
    BeginX: FamilyX<Self>,
    DumpX: FamilyX<Self>;

#[derive(Debug, Clone, Copy)]
pub struct Parsed;
impl FamilyX<Parsed> for AstX {
    type R = ();
}
impl FamilyX<Parsed> for BoolX {
    type R = ();
}
impl FamilyX<Parsed> for IntX {
    type R = ();
}
impl FamilyX<Parsed> for StringX {
    type R = ();
}
impl FamilyX<Parsed> for NilX {
    type R = ();
}
impl FamilyX<Parsed> for QuoteX {
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
impl FamilyX<Parsed> for DumpX {
    type R = ();
}

#[derive(Debug, Clone)]
pub enum Expr<X>
where
    X: XBound,
{
    Bool(RunX<BoolX, X>, bool),
    Int(RunX<IntX, X>, i32),
    String(RunX<StringX, X>, String),
    Nil(RunX<NilX, X>),
    Quote(RunX<QuoteX, X>, SExpr),
    Define(RunX<DefineX, X>, String, Box<Expr<X>>),
    Lambda(RunX<LambdaX, X>, Lambda<X>),
    If(RunX<IfX, X>, Box<Expr<X>>, Box<Expr<X>>, Box<Expr<X>>),
    Call(RunX<CallX, X>, Box<Expr<X>>, Vec<Expr<X>>),
    Var(RunX<VarX, X>, String),
    Begin(RunX<BeginX, X>, Vec<Expr<X>>),
    Dump(RunX<DumpX, X>, Box<Expr<X>>),
}

impl Expr<Parsed> {
    fn from_sexpr(sexpr: SExpr) -> Result<Self> {
        match sexpr {
            SExpr::Bool(b) => Ok(Expr::Bool((), b)),
            SExpr::Int(i) => Ok(Expr::Int((), i)),
            SExpr::String(s) => Ok(Expr::String((), s)),
            SExpr::Symbol(s) => Ok(Expr::Var((), s)),
            SExpr::Nil => Ok(Expr::Nil(())),
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("quote"),
                cdr,
            }) => match cdr {
                list_pattern![sexpr] => Ok(Expr::Quote((), sexpr)),
                _ => Err(anyhow::anyhow!("Invalid quote expression")),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("define"),
                cdr,
            }) => match cdr {
                list_pattern![SExpr::Symbol(name), expr] => {
                    Ok(Expr::Define((), name, Box::new(Expr::from_sexpr(expr)?)))
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
                list_pattern![args, ..sexprs] => {
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
                        .collect::<Result<Vec<_>>>()?;
                    Ok(Expr::Lambda((), Lambda { args, body: exprs }))
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
                    Ok(Expr::If((), Box::new(cond), Box::new(then), Box::new(els)))
                }
                _ => Err(anyhow::anyhow!("Invalid if expression",)),
            },
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("let"),
                cdr,
            }) => match cdr {
                // TODO: 効率が悪いが一旦ラムダ式に変換
                list_pattern![bindings, ..body_sexprs] => {
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
                    let lambda = list![
                        SExpr::Symbol("lambda".to_string()),
                        SExpr::from_vec(names),
                        ..body_sexprs
                    ];

                    let exprs = SExpr::from_vec(exprs);

                    let result = list![lambda, ..exprs];
                    Expr::from_sexpr(result)
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
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Begin((), exprs))
            }
            SExpr::Cons(box Cons {
                car: SExpr::Symbol("dump"),
                cdr,
            }) => match cdr {
                list_pattern![expr] => Ok(Expr::Dump((), Box::new(Expr::from_sexpr(expr)?))),
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
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call((), Box::new(func), args))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lambda<X>
where
    X: XBound,
{
    pub args: Vec<String>,
    pub body: Vec<Expr<X>>,
}
