use crate::{
    compiler_error,
    error::CompilerError,
    span::{Pos, Span},
    token::TokenKind,
};
use nom_locate::LocatedSpan;

use super::token::Token;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{anychar, satisfy},
    combinator::{consumed, eof as nom_eof, map, map_res},
    error::{convert_error, ParseError, VerboseError},
    multi::many0,
    IResult, Parser,
};

type LocatedStr<'a> = LocatedSpan<&'a str>;

fn identifier(input: LocatedStr) -> IResult<LocatedStr, TokenKind, VerboseError<LocatedStr>> {
    const SYMBOLS: &str = "!$%&*+-/:<=>?^_~";

    let (input, first) = satisfy(|c: char| c.is_ascii_alphabetic() || SYMBOLS.contains(c))(input)?;
    let (input, rest) =
        take_while(|c: char| c.is_ascii_alphanumeric() || SYMBOLS.contains(c))(input)?;
    Ok((input, TokenKind::Identifier(format!("{}{}", first, rest))))
}

fn int(input: LocatedStr) -> IResult<LocatedStr, TokenKind, VerboseError<LocatedStr>> {
    let (input, int) = map_res(take_while(|c: char| c.is_ascii_digit()), |s: LocatedStr| {
        s.parse::<i64>()
    })(input)?;
    Ok((input, TokenKind::Int(int)))
}

fn string(input: LocatedStr) -> IResult<LocatedStr, TokenKind, VerboseError<LocatedStr>> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, TokenKind::String(string.to_string())))
}

fn char(input: LocatedStr) -> IResult<LocatedStr, TokenKind, VerboseError<LocatedStr>> {
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
                _ => Err(nom::Err::Failure(VerboseError::from_char(input, ' '))),
            }
        } else {
            Ok((input, TokenKind::Char(first)))
        }
    } else {
        Ok((input, TokenKind::Char(first)))
    }
}

fn token_kind(input: LocatedStr) -> IResult<LocatedStr, TokenKind, VerboseError<LocatedStr>> {
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
    ))
    .parse(input)
}

fn space(input: LocatedStr) -> IResult<LocatedStr, (), VerboseError<LocatedStr>> {
    map(take_while1(|c: char| c.is_ascii_whitespace()), |_| ()).parse(input)
}

fn line_comment(input: LocatedStr) -> IResult<LocatedStr, (), VerboseError<LocatedStr>> {
    let (input, _) = tag(";")(input)?;
    let (input, _) = take_while(|c: char| c != '\n')(input)?;
    Ok((input, ()))
}

fn ignore(input: LocatedStr) -> IResult<LocatedStr, (), VerboseError<LocatedStr>> {
    let (input, _) = many0(alt((space, line_comment)))(input)?;
    Ok((input, ()))
}

fn token(input: LocatedStr) -> IResult<LocatedStr, Token, VerboseError<LocatedStr>> {
    let (input, _) = ignore(input)?;
    let (input, (pos, kind)) = consumed(token_kind)(input)?;
    Ok((
        input,
        Token {
            kind,
            span: to_span(pos),
        },
    ))
}

fn eof(input: LocatedStr) -> IResult<LocatedStr, Token, VerboseError<LocatedStr>> {
    let (input, _) = ignore(input)?;
    let (input, (pos, _)) = consumed(nom_eof)(input)?;
    Ok((
        input,
        Token {
            kind: TokenKind::Eof,
            span: to_span(pos),
        },
    ))
}

fn tokens(input: LocatedStr) -> IResult<LocatedStr, Vec<Token>, VerboseError<LocatedStr>> {
    let (input, tokens) = many0(token)(input)?;
    let (input, eof_token) = eof(input)?;
    Ok((input, {
        let mut tokens = tokens;
        tokens.push(eof_token);
        tokens
    }))
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

pub fn lex(input: &str) -> Result<Vec<Token>, CompilerError> {
    let input = LocatedStr::new(input);
    let (input, tokens) = tokens(input).map_err(|e| compiler_error!("{}", e))?;
    debug_assert!(input.len() == 0);
    Ok(tokens)
}
