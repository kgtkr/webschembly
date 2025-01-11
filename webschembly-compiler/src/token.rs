#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Identifier(String),
    OpenParen,
    CloseParen,
    Number(i64),
    String(String),
    Boolean(bool),
    Character(char),
    Quote,
    Eof,
}
