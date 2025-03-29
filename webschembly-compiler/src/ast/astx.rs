use std::fmt::Debug;

use crate::sexpr::SExpr;
use crate::x::{FamilyRunX, Phase, RunX};

#[derive(Debug, Clone)]
pub enum AstX {}
#[derive(Debug, Clone)]
pub enum LiteralX {}
#[derive(Debug, Clone)]

pub enum DefineX {}
#[derive(Debug, Clone)]

pub enum LambdaX {}
#[derive(Debug, Clone)]

pub enum IfX {}
#[derive(Debug, Clone)]

pub enum CallX {}
#[derive(Debug, Clone)]

pub enum VarX {}
#[derive(Debug, Clone)]
pub enum BeginX {}
#[derive(Debug, Clone)]

pub enum SetX {}
#[derive(Debug, Clone)]

pub enum LetX {}

pub trait XBound = Sized + Phase + Clone + Debug
where
    AstX: FamilyRunX<Self>,
    LiteralX: FamilyRunX<Self>,
    DefineX: FamilyRunX<Self>,
    LambdaX: FamilyRunX<Self>,
    IfX: FamilyRunX<Self>,
    CallX: FamilyRunX<Self>,
    VarX: FamilyRunX<Self>,
    BeginX: FamilyRunX<Self>,
    SetX: FamilyRunX<Self>,
    LetX: FamilyRunX<Self>;

#[derive(Debug, Clone)]
pub struct Ast<X>
where
    X: XBound,
{
    pub x: RunX<AstX, X>,
    pub exprs: Vec<Expr<X>>,
}

#[derive(Debug, Clone)]
pub enum Expr<X>
where
    X: XBound,
{
    Literal(RunX<LiteralX, X>, Literal),
    Var(RunX<VarX, X>, String),
    Define(RunX<DefineX, X>, Define<X>),
    Lambda(RunX<LambdaX, X>, Lambda<X>),
    If(RunX<IfX, X>, If<X>),
    Call(RunX<CallX, X>, Call<X>),
    Begin(RunX<BeginX, X>, Begin<X>),
    Set(RunX<SetX, X>, Set<X>),
    Let(RunX<LetX, X>, Let<X>),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Bool(bool),
    Int(i64),
    String(String),
    Nil,
    Quote(SExpr),
    Char(char),
}

#[derive(Debug, Clone)]
pub struct Define<X>
where
    X: XBound,
{
    pub name: String,
    pub expr: Box<Expr<X>>,
}

#[derive(Debug, Clone)]
pub struct Lambda<X>
where
    X: XBound,
{
    pub args: Vec<String>,
    pub body: Vec<Expr<X>>,
}

#[derive(Debug, Clone)]
pub struct If<X>
where
    X: XBound,
{
    pub cond: Box<Expr<X>>,
    pub then: Box<Expr<X>>,
    pub els: Box<Expr<X>>,
}

#[derive(Debug, Clone)]
pub struct Call<X>
where
    X: XBound,
{
    pub func: Box<Expr<X>>,
    pub args: Vec<Expr<X>>,
}

#[derive(Debug, Clone)]
pub struct Begin<X>
where
    X: XBound,
{
    pub exprs: Vec<Expr<X>>,
}

#[derive(Debug, Clone)]
pub struct Set<X>
where
    X: XBound,
{
    pub name: String,
    pub expr: Box<Expr<X>>,
}

#[derive(Debug, Clone)]
pub struct Let<X>
where
    X: XBound,
{
    pub bindings: Vec<(String, Expr<X>)>,
    pub body: Vec<Expr<X>>,
}
