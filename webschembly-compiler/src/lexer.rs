use crate::{parser_combinator::many_until, span::Span, token::TokenKind};

use super::token::Token;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{anychar, satisfy},
    combinator::{consumed, eof, map, map_res},
    multi::many0,
    IResult, Parser,
};

fn identifier(input: Span) -> IResult<Span, TokenKind> {
    const SYMBOLS: &str = "!$%&*+-/:<=>?^_~";

    let (input, first) = satisfy(|c: char| c.is_ascii_alphabetic() || SYMBOLS.contains(c))(input)?;
    let (input, rest) =
        take_while(|c: char| c.is_ascii_alphanumeric() || SYMBOLS.contains(c))(input)?;
    Ok((input, TokenKind::Identifier(format!("{}{}", first, rest))))
}

fn int(input: Span) -> IResult<Span, TokenKind> {
    let (input, int) = map_res(take_while(|c: char| c.is_ascii_digit()), |s: Span| {
        s.parse::<i64>()
    })(input)?;
    Ok((input, TokenKind::Int(int)))
}

fn string(input: Span) -> IResult<Span, TokenKind> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, TokenKind::String(string.to_string())))
}

fn char(input: Span) -> IResult<Span, TokenKind> {
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

fn token_kind(input: Span) -> IResult<Span, TokenKind> {
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

fn space(input: Span) -> IResult<Span, ()> {
    map(take_while1(|c: char| c.is_ascii_whitespace()), |_| ()).parse(input)
}

fn line_comment(input: Span) -> IResult<Span, ()> {
    let (input, _) = tag(";")(input)?;
    let (input, _) = take_while(|c: char| c != '\n')(input)?;
    Ok((input, ()))
}

fn ignore(input: Span) -> IResult<Span, ()> {
    let (input, _) = many0(alt((space, line_comment)))(input)?;
    Ok((input, ()))
}

fn token(input: Span) -> IResult<Span, Token> {
    let (input, (ignore_pos, _)) = consumed(ignore)(input)?;
    let (input, (pos, kind)) = consumed(token_kind)(input)?;
    Ok((
        input,
        Token {
            kind,
            ignore_pos,
            pos,
        },
    ))
}

fn tokens(input: Span) -> IResult<Span, Vec<Token>> {
    let (input, tokens) = many_until(token, |token| token.kind == TokenKind::Eof)(input)?;
    Ok((input, tokens))
}

pub fn lex(input: &str) -> Result<Vec<Token>, nom::Err<nom::error::Error<Span>>> {
    let input = Span::new(input);
    let (input, tokens) = tokens(input)?;
    debug_assert!(input.len() == 0);
    Ok(tokens)
}
