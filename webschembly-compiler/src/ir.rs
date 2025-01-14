use crate::ast::AST;

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    String(String),
    Nil,
    Cons(usize, usize),
    MutCell(usize),
    MutCellDeref(usize),
    Call(usize, Vec<usize>),
}

#[derive(Debug, Clone)]
pub enum Stat {
    If(Box<Stat>, Box<Stat>, Box<Stat>),
    Begin(Vec<Stat>),
    Expr(Option<usize>, Expr),
}

#[derive(Debug, Clone)]
pub struct Func {
    pub args: usize,
    pub locals: usize,
    pub ret: usize,
    pub body: Stat,
}
