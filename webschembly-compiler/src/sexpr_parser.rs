use crate::sexpr::{Cons, NonEmptyList};

use super::sexpr::SExpr;
use super::token::Token;

use super::parser_combinator::{satisfy, satisfy_map_opt};
use nom::combinator::eof;
use nom::{branch::alt, multi::many0, IResult, Parser};

fn bool(input: &[Token]) -> IResult<&[Token], SExpr> {
    satisfy_map_opt(|t| match t {
        Token::Bool(b) => Some(SExpr::Bool(*b)),
        _ => None,
    })
    .parse(input)
}

fn int(input: &[Token]) -> IResult<&[Token], SExpr> {
    satisfy_map_opt(|t| match t {
        Token::Int(i) => Some(SExpr::Int(*i)),
        _ => None,
    })
    .parse(input)
}

fn string(input: &[Token]) -> IResult<&[Token], SExpr> {
    satisfy_map_opt(|t| match t {
        Token::String(s) => Some(SExpr::String(s.clone())),
        _ => None,
    })
    .parse(input)
}

fn symbol(input: &[Token]) -> IResult<&[Token], SExpr> {
    satisfy_map_opt(|t| match t {
        Token::Identifier(s) => Some(SExpr::Symbol(s.clone())),
        _ => None,
    })
    .parse(input)
}

fn char(input: &[Token]) -> IResult<&[Token], SExpr> {
    satisfy_map_opt(|t| match t {
        Token::Char(c) => Some(SExpr::Char(*c)),
        _ => None,
    })
    .parse(input)
}

fn nil(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::CloseParen).parse(input)?;
    Ok((input, SExpr::Nil))
}

fn list_or_dotted_list(input: &[Token]) -> IResult<&[Token], SExpr> {
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

fn list(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::CloseParen).parse(input)?;
    Ok((input, SExpr::Nil))
}

fn dotted_list(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::Dot).parse(input)?;
    let (input, cdr) = sexpr(input)?;
    let (input, _) = satisfy(|t| *t == Token::CloseParen).parse(input)?;
    Ok((input, cdr))
}

fn nil_or_list_or_dotted_list(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::OpenParen).parse(input)?;
    alt((nil, list_or_dotted_list)).parse(input)
}

fn quote(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::Quote).parse(input)?;
    let (input, expr) = sexpr(input)?;
    Ok((input, list![SExpr::Symbol("quote".to_string()), expr]))
}

fn sexpr(input: &[Token]) -> IResult<&[Token], SExpr> {
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

fn sexprs(input: &[Token]) -> IResult<&[Token], Vec<SExpr>> {
    let (input, sexprs) = many0(sexpr).parse(input)?;
    Ok((input, sexprs))
}

pub fn parse(input: &[Token]) -> Result<Vec<SExpr>, nom::Err<nom::error::Error<&[Token]>>> {
    let (input, sexprs) = sexprs(input)?;
    let (_, _) = eof(input)?;
    Ok(sexprs)
}
