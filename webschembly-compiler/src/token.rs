use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPayload {
    Identifier(String),
    OpenParen,
    CloseParen,
    Int(i64),
    String(String),
    Bool(bool),
    Quote,
    Dot,
    Eof,
    Char(char),
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub payload: TokenPayload,
    pub pos: Span<'a>,
}
