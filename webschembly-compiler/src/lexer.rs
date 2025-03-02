use crate::{
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
    error::{ErrorKind, FromExternalError, ParseError, VerboseError, VerboseErrorKind},
    multi::many0,
    Finish, IResult, Parser,
};
use std::fmt::Write;

type LocatedStr<'a> = LocatedSpan<&'a str>;
trait ErrorBound<'a> = ParseError<LocatedStr<'a>> + FromExternalError<LocatedStr<'a>, Self>;

fn identifier<'a, E: ErrorBound<'a>>(
    input: LocatedStr<'a>,
) -> IResult<LocatedStr<'a>, TokenKind, E> {
    const SYMBOLS: &str = "!$%&*+-/:<=>?^_~";

    let (input, first) = satisfy(|c: char| c.is_ascii_alphabetic() || SYMBOLS.contains(c))(input)?;
    let (input, rest) =
        take_while(|c: char| c.is_ascii_alphanumeric() || SYMBOLS.contains(c))(input)?;
    Ok((input, TokenKind::Identifier(format!("{}{}", first, rest))))
}

fn int<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, int) = map_res(take_while(|c: char| c.is_ascii_digit()), |s: LocatedStr| {
        s.parse::<i64>()
            .map_err(|_| E::from_error_kind(s, ErrorKind::Digit))
    })(input)?;
    Ok((input, TokenKind::Int(int)))
}

fn string<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, TokenKind::String(string.to_string())))
}

fn char<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
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
                _ => Err(nom::Err::Failure(E::from_error_kind(
                    input,
                    ErrorKind::Char,
                ))),
            }
        } else {
            Ok((input, TokenKind::Char(first)))
        }
    } else {
        Ok((input, TokenKind::Char(first)))
    }
}

fn token_kind<'a, E: ErrorBound<'a>>(
    input: LocatedStr<'a>,
) -> IResult<LocatedStr<'a>, TokenKind, E> {
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

fn space<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, (), E> {
    map(take_while1(|c: char| c.is_ascii_whitespace()), |_| ()).parse(input)
}

fn line_comment<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, (), E> {
    let (input, _) = tag(";")(input)?;
    let (input, _) = take_while(|c: char| c != '\n')(input)?;
    Ok((input, ()))
}

fn ignore<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, (), E> {
    let (input, _) = many0(alt((space, line_comment)))(input)?;
    Ok((input, ()))
}

fn token<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, Token, E> {
    let (input, _) = ignore(input)?;
    let (input, (pos, kind)) = consumed(token_kind)(input)?;
    Ok((
        input,
        Token {
            kind,
            span: to_span(&pos),
        },
    ))
}

fn eof<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, Token, E> {
    let (input, _) = ignore(input)?;
    let (input, (pos, _)) = consumed(nom_eof)(input)?;
    Ok((
        input,
        Token {
            kind: TokenKind::Eof,
            span: to_span(&pos),
        },
    ))
}

fn tokens<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, Vec<Token>, E> {
    let (input, tokens) = many0(token)(input)?;
    let (input, eof_token) = eof(input)?;
    Ok((input, {
        let mut tokens = tokens;
        tokens.push(eof_token);
        tokens
    }))
}

// トークンが複数行にまたがることはないという前提
fn to_span(located: &LocatedStr) -> Span {
    let start = to_pos(located);
    let end = Pos::new(
        start.line,
        start.column + located.fragment().chars().count(),
    );
    Span::new(start, end)
}

fn to_pos(located: &LocatedStr) -> Pos {
    Pos::new(
        located.location_line() as usize,
        located.get_utf8_column() as usize,
    )
}

fn convert_error(e: VerboseError<LocatedStr>) -> CompilerError {
    let mut result = String::new();

    for (substring, kind) in e.errors.iter() {
        let pos = to_pos(substring);

        match kind {
            VerboseErrorKind::Char(c) => {
                if let Some(actual) = substring.chars().next() {
                    write!(
                        &mut result,
                        "{pos}: expected '{expected}', found {actual}\n",
                        pos = pos,
                        expected = c,
                        actual = actual,
                    )
                } else {
                    write!(
                        &mut result,
                        "{pos}: expected '{expected}', got end of input\n",
                        pos = pos,
                        expected = c,
                    )
                }
            }
            VerboseErrorKind::Context(s) => write!(
                &mut result,
                "{pos}, in {context}:\n",
                pos = pos,
                context = s,
            ),
            VerboseErrorKind::Nom(e) => write!(
                &mut result,
                "{pos}, in {nom_err:?}:\n",
                pos = pos,
                nom_err = e,
            ),
        }
        .unwrap();
    }

    CompilerError(result)
}

pub fn lex(input: &str) -> Result<Vec<Token>, CompilerError> {
    let input = LocatedStr::new(input);
    let (input, tokens) = tokens::<nom::error::VerboseError<_>>(input)
        .finish()
        .map_err(convert_error)?;
    debug_assert!(input.len() == 0);
    Ok(tokens)
}

#[test]
fn test_lex() {
    use insta::assert_debug_snapshot;
    let result = lex("(+ 1 2)").unwrap();
    assert_debug_snapshot!(result);
}
