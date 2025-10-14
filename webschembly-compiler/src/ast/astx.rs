use std::fmt::Debug;

use ordered_float::NotNan;

use crate::sexpr::SExpr;
use crate::span::Span;
use crate::x::{FamilyX, Phase, RunX};

#[derive(Debug, Clone)]
pub struct Located<T> {
    pub value: T,
    pub span: Span,
}

pub type L<T> = Located<T>;
pub type LExpr<X> = L<Expr<X>>;

pub trait LocatedValue: Sized {
    fn with_span(self, span: Span) -> L<Self> {
        L { value: self, span }
    }
}

impl<T> LocatedValue for T {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UVectorKind {
    S64,
    F64,
}

#[derive(Debug, Clone)]
pub enum AstX {}
#[derive(Debug, Clone)]
pub enum ConstX {}
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
#[derive(Debug, Clone)]
pub enum LetRecX {}

#[derive(Debug, Clone)]

pub enum VectorX {}

#[derive(Debug, Clone)]
pub enum UVectorX {}

#[derive(Debug, Clone)]

pub enum QuoteX {}

#[derive(Debug, Clone)]
pub enum ConsX {}

pub trait XBound = Sized + Phase + Clone + Debug
where
    AstX: FamilyX<Self>,
    ConstX: FamilyX<Self>,
    DefineX: FamilyX<Self>,
    LambdaX: FamilyX<Self>,
    IfX: FamilyX<Self>,
    CallX: FamilyX<Self>,
    VarX: FamilyX<Self>,
    BeginX: FamilyX<Self>,
    SetX: FamilyX<Self>,
    LetX: FamilyX<Self>,
    LetRecX: FamilyX<Self>,
    VectorX: FamilyX<Self>,
    UVectorX: FamilyX<Self>,
    QuoteX: FamilyX<Self>,
    ConsX: FamilyX<Self>;

#[derive(Debug, Clone)]
pub struct Ast<X>
where
    X: XBound,
{
    pub x: RunX<AstX, X>,
    pub exprs: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub enum Expr<X>
where
    X: XBound,
{
    Const(RunX<ConstX, X>, Const),
    Var(RunX<VarX, X>, String),
    Define(RunX<DefineX, X>, Define<X>),
    Lambda(RunX<LambdaX, X>, Lambda<X>),
    If(RunX<IfX, X>, If<X>),
    Call(RunX<CallX, X>, Call<X>),
    Begin(RunX<BeginX, X>, Begin<X>),
    Set(RunX<SetX, X>, Set<X>),
    Let(RunX<LetX, X>, Let<X>),
    LetRec(RunX<LetRecX, X>, LetRec<X>),
    Vector(RunX<VectorX, X>, Vec<Vec<LExpr<X>>>),
    UVector(RunX<UVectorX, X>, UVector<X>),
    Quote(RunX<QuoteX, X>, SExpr),
    Cons(RunX<ConsX, X>, Cons<X>),
}

#[derive(Debug, Clone)]
pub enum Const {
    Bool(bool),
    Int(i64),
    Float(NotNan<f64>),
    NaN,
    String(String),
    Nil,
    Char(char),
    Symbol(String),
}

#[derive(Debug, Clone)]
pub struct Define<X>
where
    X: XBound,
{
    pub name: L<String>,
    pub expr: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Lambda<X>
where
    X: XBound,
{
    pub args: Vec<L<String>>,
    pub body: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct If<X>
where
    X: XBound,
{
    pub cond: Vec<LExpr<X>>,
    pub then: Vec<LExpr<X>>,
    pub els: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Call<X>
where
    X: XBound,
{
    pub func: Vec<LExpr<X>>,
    pub args: Vec<Vec<LExpr<X>>>,
}

#[derive(Debug, Clone)]
pub struct Begin<X>
where
    X: XBound,
{
    pub exprs: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Set<X>
where
    X: XBound,
{
    pub name: L<String>,
    pub expr: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Let<X>
where
    X: XBound,
{
    pub bindings: Vec<L<Binding<X>>>,
    pub body: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct LetRec<X>
where
    X: XBound,
{
    pub bindings: Vec<L<Binding<X>>>,
    pub body: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Binding<X>
where
    X: XBound,
{
    pub name: L<String>,
    pub expr: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct UVector<X>
where
    X: XBound,
{
    pub kind: UVectorKind,
    pub elements: Vec<Vec<LExpr<X>>>,
}

#[derive(Debug, Clone)]
pub struct Cons<X>
where
    X: XBound,
{
    pub car: Vec<LExpr<X>>,
    pub cdr: Vec<LExpr<X>>,
}
