use crate::sexpr::SExpr;
use crate::x::{FamilyX, RunX};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub enum AstX {}
pub enum LiteralX {}

pub enum DefineX {}

pub enum LambdaX {}

pub enum IfX {}

pub enum CallX {}

pub enum VarX {}

pub enum BeginX {}

pub enum SetX {}

pub trait XBound = Sized
where
    AstX: FamilyX<Self>,
    LiteralX: FamilyX<Self>,
    DefineX: FamilyX<Self>,
    LambdaX: FamilyX<Self>,
    IfX: FamilyX<Self>,
    CallX: FamilyX<Self>,
    VarX: FamilyX<Self>,
    BeginX: FamilyX<Self>,
    SetX: FamilyX<Self>;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum Builtin {
    Display,
    Add,
    WriteChar,
}

impl Builtin {
    pub fn name(self) -> &'static str {
        match self {
            Builtin::Display => "display",
            Builtin::Add => "+",
            Builtin::WriteChar => "write-char",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Builtin::iter().find(|&builtin| builtin.name() == name)
    }

    pub fn id(self) -> i32 {
        match self {
            Builtin::Display => 0,
            Builtin::Add => 1,
            Builtin::WriteChar => 2,
        }
    }

    pub fn from_id(id: i32) -> Option<Self> {
        Builtin::iter().find(|&builtin| builtin.id() == id)
    }
}
