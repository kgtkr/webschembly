use std::fmt::Debug;

use ordered_float::NotNan;

use crate::sexpr::SExpr;
use webschembly_compiler_locate::L;

pub type LExpr<X> = L<Expr<X>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UVectorKind {
    S64,
    F64,
}

pub trait AstPhase: Sized + Clone + Debug {
    type XAst: std::fmt::Debug + Clone = ();
    type XConst: std::fmt::Debug + Clone = ();
    type XDefine: std::fmt::Debug + Clone = ();
    type XLambda: std::fmt::Debug + Clone = ();
    type XIf: std::fmt::Debug + Clone = ();
    type XCall: std::fmt::Debug + Clone = ();
    type XVar: std::fmt::Debug + Clone = ();
    type XBegin: std::fmt::Debug + Clone = ();
    type XSet: std::fmt::Debug + Clone = ();
    type XLet: std::fmt::Debug + Clone = ();
    type XLetRec: std::fmt::Debug + Clone = ();
    type XVector: std::fmt::Debug + Clone = ();
    type XUVector: std::fmt::Debug + Clone = ();
    type XQuote: std::fmt::Debug + Clone = ();
    type XCons: std::fmt::Debug + Clone = ();
}

#[derive(Debug, Clone)]
pub struct Ast<X>
where
    X: AstPhase,
{
    pub x: X::XAst,
    pub exprs: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub enum Expr<X>
where
    X: AstPhase,
{
    Const(X::XConst, Const),
    Var(X::XVar, String),
    Define(X::XDefine, Define<X>),
    Lambda(X::XLambda, Lambda<X>),
    If(X::XIf, If<X>),
    Call(X::XCall, Call<X>),
    Begin(X::XBegin, Begin<X>),
    Set(X::XSet, Set<X>),
    Let(X::XLet, Let<X>),
    LetRec(X::XLetRec, LetRec<X>),
    Vector(X::XVector, Vec<Vec<LExpr<X>>>),
    UVector(X::XUVector, UVector<X>),
    Quote(X::XQuote, SExpr),
    Cons(X::XCons, Cons<X>),
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
    X: AstPhase,
{
    pub name: L<String>,
    pub expr: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Lambda<X>
where
    X: AstPhase,
{
    pub args: Vec<L<String>>,
    pub body: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct If<X>
where
    X: AstPhase,
{
    pub cond: Vec<LExpr<X>>,
    pub then: Vec<LExpr<X>>,
    pub els: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Call<X>
where
    X: AstPhase,
{
    pub func: Vec<LExpr<X>>,
    pub args: Vec<Vec<LExpr<X>>>,
}

#[derive(Debug, Clone)]
pub struct Begin<X>
where
    X: AstPhase,
{
    pub exprs: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Set<X>
where
    X: AstPhase,
{
    pub name: L<String>,
    pub expr: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Let<X>
where
    X: AstPhase,
{
    pub bindings: Vec<L<Binding<X>>>,
    pub body: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct LetRec<X>
where
    X: AstPhase,
{
    pub bindings: Vec<L<Binding<X>>>,
    pub body: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Binding<X>
where
    X: AstPhase,
{
    pub name: L<String>,
    pub expr: Vec<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct UVector<X>
where
    X: AstPhase,
{
    pub kind: UVectorKind,
    pub elements: Vec<Vec<LExpr<X>>>,
}

#[derive(Debug, Clone)]
pub struct Cons<X>
where
    X: AstPhase,
{
    pub car: Vec<LExpr<X>>,
    pub cdr: Vec<LExpr<X>>,
}
