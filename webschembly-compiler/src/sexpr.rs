#[derive(Debug, Clone)]
pub enum SExpr {
    Bool(bool),
    Int(i64),
    String(String),
    Symbol(String),
    List(Vec<SExpr>),
    Quote(Box<SExpr>),
    // cdr is not list
    DottedList(Vec<SExpr>, Box<SExpr>),
}
