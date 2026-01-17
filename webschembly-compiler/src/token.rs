use ordered_float::NotNan;

use webschembly_compiler_locate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    OpenParen,
    CloseParen,
    Int(i64),
    Float(NotNan<f64>),
    NaN,
    String(String),
    Bool(bool),
    Quote,
    VectorOpenParen,
    UVectorS64OpenParen,
    UVectorF64OpenParen,
    Dot,
    Eof,
    Char(char),
    Directive(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
