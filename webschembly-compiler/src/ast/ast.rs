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
    Quote(RunX<QuoteX, X>, SExpr),
    Define(RunX<DefineX, X>, String, Box<Expr<X>>),
    Lambda(RunX<LambdaX, X>, Lambda<X>),
    If(RunX<IfX, X>, Box<Expr<X>>, Box<Expr<X>>, Box<Expr<X>>),
    Call(RunX<CallX, X>, Box<Expr<X>>, Vec<Expr<X>>),
    Var(RunX<VarX, X>, String),
    Begin(RunX<BeginX, X>, Vec<Expr<X>>),
    Dump(RunX<DumpX, X>, Box<Expr<X>>),
}

#[derive(Debug, Clone)]
pub struct Lambda<X>
where
    X: XBound,
{
    pub args: Vec<String>,
    pub body: Vec<Expr<X>>,
}
