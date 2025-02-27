use crate::sexpr::{Cons, NonEmptyList};
use crate::token::TokenKind;

use super::sexpr::SExpr;
use super::token::Token;

use super::parser_combinator::{satisfy, satisfy_map_opt};
use nom::{branch::alt, multi::many0, IResult, Parser};

type Tokens<'a> = &'a [Token<'a>];

fn bool(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Bool(b) => Some(SExpr::Bool(*b)),
        _ => None,
    })
    .parse(input)
}

fn int(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Int(i) => Some(SExpr::Int(*i)),
        _ => None,
    })
    .parse(input)
}

fn string(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::String(s) => Some(SExpr::String(s.clone())),
        _ => None,
    })
    .parse(input)
}

fn symbol(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Identifier(s) => Some(SExpr::Symbol(s.clone())),
        _ => None,
    })
    .parse(input)
}

fn char(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Char(c) => Some(SExpr::Char(*c)),
        _ => None,
    })
    .parse(input)
}

fn nil(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;
    Ok((input, SExpr::Nil))
}

fn list_or_dotted_list(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, first) = sexpr(input)?;
    let (input, middle) = many0(sexpr).parse(input)?;
    let (input, last) = alt((list, dotted_list)).parse(input)?;

    Ok((
        input,
        SExpr::Cons(Box::new(Cons::from_non_empty_list(NonEmptyList::new(
            first, middle, last,
        )))),
    ))
}

fn list(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;
    Ok((input, SExpr::Nil))
}

fn dotted_list(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::Dot).parse(input)?;
    let (input, cdr) = sexpr(input)?;
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;
    Ok((input, cdr))
}

fn nil_or_list_or_dotted_list(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::OpenParen).parse(input)?;
    alt((nil, list_or_dotted_list)).parse(input)
}

fn quote(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::Quote).parse(input)?;
    let (input, expr) = sexpr(input)?;
    Ok((input, list![SExpr::Symbol("quote".to_string()), expr]))
}

fn sexpr(input: Tokens) -> IResult<Tokens, SExpr> {
    alt((
        bool,
        int,
        string,
        symbol,
        char,
        nil_or_list_or_dotted_list,
        quote,
    ))
    .parse(input)
}

fn sexprs(input: Tokens) -> IResult<Tokens, Vec<SExpr>> {
    let (input, sexprs) = many0(sexpr).parse(input)?;
    Ok((input, sexprs))
}

pub fn parse(input: Tokens) -> Result<Vec<SExpr>, nom::Err<nom::error::Error<Tokens>>> {
    let (input, sexprs) = sexprs(input)?;
    let (_, _) = satisfy(|t: &Token| t.kind == TokenKind::Eof).parse(input)?;
    Ok(sexprs)
}
