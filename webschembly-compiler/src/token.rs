use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
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
    pub kind: TokenKind,
    pub pos: Span<'a>,
}
