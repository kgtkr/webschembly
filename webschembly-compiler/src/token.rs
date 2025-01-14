#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    OpenParen,
    CloseParen,
    Int(i32),
    String(String),
    Bool(bool),
    Quote,
    Dot,
    Eof,
}
