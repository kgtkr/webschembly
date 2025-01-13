use super::sexpr::SExpr;
use super::token::Token;

use super::parser_combinator::{satisfy, satisfy_map_opt};
use nom::{branch::alt, error::ParseError, multi::many0, IResult, Parser};

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

fn list_rest<'a, E: ParseError<&'a [Token]>>(
    list: Vec<SExpr>,
) -> impl Parser<&'a [Token], SExpr, E> {
    move |input: &'a [Token]| {
        let (input, _) = satisfy(|t| *t == Token::CloseParen).parse(input)?;
        Ok((input, SExpr::List(list.clone())))
    }
}

fn dotted_list_rest<'a>(
    list: Vec<SExpr>,
) -> impl Parser<&'a [Token], SExpr, nom::error::Error<&'a [Token]>> {
    move |input: &'a [Token]| {
        let (input, _) = satisfy(|t| *t == Token::Dot).parse(input)?;
        let (input, cdr) = sexpr(input)?;
        let (input, _) = satisfy(|t| *t == Token::CloseParen).parse(input)?;
        match cdr {
            SExpr::List(cdr_list) => Ok((
                input,
                SExpr::List({
                    let mut list = list.clone();
                    list.extend(cdr_list);
                    list
                }),
            )),
            cdr => Ok((input, SExpr::DottedList(list.clone(), Box::new(cdr)))),
        }
    }
}

fn list_or_dotted_list(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::OpenParen).parse(input)?;
    let (input, list) = many0(sexpr).parse(input)?;
    alt((list_rest(list.clone()), dotted_list_rest(list))).parse(input)
}

fn quote(input: &[Token]) -> IResult<&[Token], SExpr> {
    let (input, _) = satisfy(|t| *t == Token::Quote).parse(input)?;
    let (input, expr) = sexpr(input)?;
    Ok((input, list![SExpr::Symbol("quote".to_string()), expr]))
}

pub fn sexpr(input: &[Token]) -> IResult<&[Token], SExpr> {
    alt((bool, int, string, symbol, list_or_dotted_list, quote)).parse(input)
}
