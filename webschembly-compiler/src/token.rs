#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    OpenParen,
    CloseParen,
    Int(i64),
    String(String),
    Bool(bool),
    Quote,
    Dot,
    Eof,
}
