#[derive(Debug, Clone)]
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
