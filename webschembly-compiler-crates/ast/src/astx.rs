use std::fmt::Debug;

use ordered_float::NotNan;

use webschembly_compiler_locate::L;
use webschembly_compiler_sexpr::LSExpr;

pub type LExpr<X> = L<Expr<X>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UVectorKind {
    S64,
    F64,
}

pub trait AstPhaseX = std::fmt::Debug + Clone;

// 字面上ではBox<T>に相当するが、beginのdesugerによってVec<T>に変換される可能性がある場所を表す
// 本当はAstPhaseで抽象化したいが、generric関連型を使うのは複雑すぎるので
pub type ExprBox<T> = Vec<T>;

pub trait AstPhase: Sized + Clone + Debug {
    type XAst: AstPhaseX;
    type XConst: AstPhaseX;
    type XDefine: AstPhaseX;
    type XLambda: AstPhaseX;
    type XIf: AstPhaseX;
    type XCall: AstPhaseX;
    type XVar: AstPhaseX;
    type XBegin: AstPhaseX;
    type XSet: AstPhaseX;
    type XLet: AstPhaseX;
    type XLetStar: AstPhaseX;
    type XLetRec: AstPhaseX;
    type XVector: AstPhaseX;
    type XUVector: AstPhaseX;
    type XQuote: AstPhaseX;
    type XCons: AstPhaseX;
    type XExt: AstPhaseX;
}

pub trait ExtendAstPhase: Sized + Clone + Debug {
    type Prev: AstPhase;
    type XAst: AstPhaseX = <Self::Prev as AstPhase>::XAst;
    type XConst: AstPhaseX = <Self::Prev as AstPhase>::XConst;
    type XDefine: AstPhaseX = <Self::Prev as AstPhase>::XDefine;
    type XLambda: AstPhaseX = <Self::Prev as AstPhase>::XLambda;
    type XIf: AstPhaseX = <Self::Prev as AstPhase>::XIf;
    type XCall: AstPhaseX = <Self::Prev as AstPhase>::XCall;
    type XVar: AstPhaseX = <Self::Prev as AstPhase>::XVar;
    type XBegin: AstPhaseX = <Self::Prev as AstPhase>::XBegin;
    type XSet: AstPhaseX = <Self::Prev as AstPhase>::XSet;
    type XLet: AstPhaseX = <Self::Prev as AstPhase>::XLet;
    type XLetStar: AstPhaseX = <Self::Prev as AstPhase>::XLetStar;
    type XLetRec: AstPhaseX = <Self::Prev as AstPhase>::XLetRec;
    type XVector: AstPhaseX = <Self::Prev as AstPhase>::XVector;
    type XUVector: AstPhaseX = <Self::Prev as AstPhase>::XUVector;
    type XQuote: AstPhaseX = <Self::Prev as AstPhase>::XQuote;
    type XCons: AstPhaseX = <Self::Prev as AstPhase>::XCons;
    type XExt: AstPhaseX = <Self::Prev as AstPhase>::XExt;
}

impl<T: ExtendAstPhase> AstPhase for T {
    type XAst = T::XAst;
    type XConst = T::XConst;
    type XDefine = T::XDefine;
    type XLambda = T::XLambda;
    type XIf = T::XIf;
    type XCall = T::XCall;
    type XVar = T::XVar;
    type XBegin = T::XBegin;
    type XSet = T::XSet;
    type XLet = T::XLet;
    type XLetStar = T::XLetStar;
    type XLetRec = T::XLetRec;
    type XVector = T::XVector;
    type XUVector = T::XUVector;
    type XQuote = T::XQuote;
    type XCons = T::XCons;
    type XExt = T::XExt;
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
    Let(X::XLet, LetLike<X>),
    LetStar(X::XLetStar, LetLike<X>),
    LetRec(X::XLetRec, LetLike<X>),
    Vector(X::XVector, Vec<ExprBox<LExpr<X>>>),
    UVector(X::XUVector, UVector<X>),
    Quote(X::XQuote, LSExpr),
    Cons(X::XCons, Cons<X>),
    Ext(X::XExt),
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
    pub expr: ExprBox<LExpr<X>>,
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
    pub cond: ExprBox<LExpr<X>>,
    pub then: ExprBox<LExpr<X>>,
    pub els: ExprBox<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct Call<X>
where
    X: AstPhase,
{
    pub func: ExprBox<LExpr<X>>,
    pub args: Vec<ExprBox<LExpr<X>>>,
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
    pub expr: ExprBox<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct LetLike<X>
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
    pub expr: ExprBox<LExpr<X>>,
}

#[derive(Debug, Clone)]
pub struct UVector<X>
where
    X: AstPhase,
{
    pub kind: UVectorKind,
    pub elements: Vec<ExprBox<LExpr<X>>>,
}

#[derive(Debug, Clone)]
pub struct Cons<X>
where
    X: AstPhase,
{
    pub car: ExprBox<LExpr<X>>,
    pub cdr: ExprBox<LExpr<X>>,
}
