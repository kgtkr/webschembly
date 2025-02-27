use crate::{span::Span, token::TokenKind};

use super::token::Token;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{anychar, satisfy},
    combinator::{consumed, eof as nom_eof, map, map_res},
    multi::many0,
    IResult, Parser,
};
use nom_locate::position;

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

fn token(input: Span) -> IResult<Span, Token> {
    let (input, (pos, kind)) = consumed(alt((
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
    )))
    .parse(input)?;
    Ok((input, Token { kind, pos }))
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

fn token_and_ignore(input: Span) -> IResult<Span, Token> {
    let (input, token) = token(input)?;
    let (input, _) = ignore(input)?;
    Ok((input, token))
}

fn eof(input: Span) -> IResult<Span, Token> {
    let (input, pos) = position(input)?;
    let (input, _) = nom_eof(input)?;
    Ok((
        input,
        Token {
            kind: TokenKind::Eof,
            pos,
        },
    ))
}

fn tokens(input: Span) -> IResult<Span, Vec<Token>> {
    let (input, _) = ignore(input)?;
    let (input, tokens) = many0(token_and_ignore)(input)?;
    let (input, eof) = eof(input)?;
    Ok((input, {
        let mut tokens = tokens;
        tokens.push(eof);
        tokens
    }))
}

pub fn lex(input: &str) -> Result<Vec<Token>, nom::Err<nom::error::Error<Span>>> {
    let input = Span::new(input);
    let (input, tokens) = tokens(input)?;
    let (_, _) = eof(input)?;
    Ok(tokens)
}
