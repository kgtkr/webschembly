use crate::{
    parser_combinator::many_until,
    span::{Pos, Span},
    token::TokenKind,
};
use nom_locate::LocatedSpan;

use super::token::Token;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{anychar, satisfy},
    combinator::{consumed, eof, map, map_res},
    multi::many0,
    IResult, Parser,
};

type LocatedStr<'a> = LocatedSpan<&'a str>;

fn identifier(input: LocatedStr) -> IResult<LocatedStr, TokenKind> {
    const SYMBOLS: &str = "!$%&*+-/:<=>?^_~";

    let (input, first) = satisfy(|c: char| c.is_ascii_alphabetic() || SYMBOLS.contains(c))(input)?;
    let (input, rest) =
        take_while(|c: char| c.is_ascii_alphanumeric() || SYMBOLS.contains(c))(input)?;
    Ok((input, TokenKind::Identifier(format!("{}{}", first, rest))))
}

fn int(input: LocatedStr) -> IResult<LocatedStr, TokenKind> {
    let (input, int) = map_res(take_while(|c: char| c.is_ascii_digit()), |s: LocatedStr| {
        s.parse::<i64>()
    })(input)?;
    Ok((input, TokenKind::Int(int)))
}

fn string(input: LocatedStr) -> IResult<LocatedStr, TokenKind> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, TokenKind::String(string.to_string())))
}

fn char(input: LocatedStr) -> IResult<LocatedStr, TokenKind> {
    let (input, _) = tag("#\\")(input)?;
    let (input, first) = anychar(input)?;
    if first.is_ascii_alphabetic() {
        let (input, rest) = take_while(|c: char| c.is_alphanumeric())(input)?;
        if !rest.is_empty() {
            let cname = format!("{}{}", first, rest);
            match cname.as_str().to_lowercase().as_str() {
                "space" => Ok((input, TokenKind::Char(' '))),
                "newline" => Ok((input, TokenKind::Char('\n'))),
                // r5rsにもgoshにもないがこれがないと括弧の対応が分かりにくくて書きにくいので
                "openparen" => Ok((input, TokenKind::Char('('))),
                "closeparen" => Ok((input, TokenKind::Char(')'))),
                _ => Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Char,
                ))),
            }
        } else {
            Ok((input, TokenKind::Char(first)))
        }
    } else {
        Ok((input, TokenKind::Char(first)))
    }
}

fn token_kind(input: LocatedStr) -> IResult<LocatedStr, TokenKind> {
    alt((
        tag("(").map(|_| TokenKind::OpenParen),
        tag(")").map(|_| TokenKind::CloseParen),
        tag("#t").map(|_| TokenKind::Bool(true)),
        tag("#f").map(|_| TokenKind::Bool(false)),
        tag("'").map(|_| TokenKind::Quote),
        tag(".").map(|_| TokenKind::Dot),
        identifier,
        int,
        string,
        char,
        eof.map(|_| TokenKind::Eof),
    ))
    .parse(input)
}

fn space(input: LocatedStr) -> IResult<LocatedStr, ()> {
    map(take_while1(|c: char| c.is_ascii_whitespace()), |_| ()).parse(input)
}

fn line_comment(input: LocatedStr) -> IResult<LocatedStr, ()> {
    let (input, _) = tag(";")(input)?;
    let (input, _) = take_while(|c: char| c != '\n')(input)?;
    Ok((input, ()))
}

fn ignore(input: LocatedStr) -> IResult<LocatedStr, ()> {
    let (input, _) = many0(alt((space, line_comment)))(input)?;
    Ok((input, ()))
}

fn token(input: LocatedStr) -> IResult<LocatedStr, (LocatedStr, TokenKind, LocatedStr)> {
    let (input, (ignore_pos, _)) = consumed(ignore)(input)?;
    let (input, (pos, kind)) = consumed(token_kind)(input)?;
    Ok((input, (ignore_pos, kind, pos)))
}

fn tokens(input: LocatedStr) -> IResult<LocatedStr, Vec<(LocatedStr, TokenKind, LocatedStr)>> {
    let (input, tokens) = many_until(token, |(_, kind, _)| kind == &TokenKind::Eof)(input)?;
    Ok((input, tokens))
}

// トークンが複数行にまたがることはないという前提
fn to_span(located: LocatedStr) -> Span {
    let start = Pos::new(
        located.location_line() as usize,
        located.get_utf8_column() as usize,
    );
    let end = Pos::new(
        start.line,
        start.column + located.fragment().chars().count(),
    );
    Span::new(start, end)
}

pub fn lex(input: &str) -> Result<Vec<Token>, nom::Err<nom::error::Error<LocatedStr>>> {
    let input = LocatedStr::new(input);
    let (input, tokens) = tokens(input)?;
    debug_assert!(input.len() == 0);
    Ok(tokens
        .into_iter()
        .map(|(_ignore_pos, token, pos)| Token {
            kind: token,
            span: to_span(pos),
        })
        .collect())
}
