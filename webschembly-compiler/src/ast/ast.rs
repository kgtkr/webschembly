use crate::sexpr::SExpr;
use crate::x::{FamilyX, RunX};

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

#[derive(Debug, Clone)]
pub struct AST<X>
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
    Bool(RunX<BoolX, X>, bool),
    Int(RunX<IntX, X>, i32),
    String(RunX<StringX, X>, String),
    Nil(RunX<NilX, X>),
    Var(RunX<VarX, X>, String),
    Quote(RunX<QuoteX, X>, SExpr),
    Define(RunX<DefineX, X>, Define<X>),
    Lambda(RunX<LambdaX, X>, Lambda<X>),
    If(RunX<IfX, X>, If<X>),
    Call(RunX<CallX, X>, Call<X>),
    Begin(RunX<BeginX, X>, Begin<X>),
    // TODO: callに統合
    Dump(RunX<DumpX, X>, Box<Expr<X>>),
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
