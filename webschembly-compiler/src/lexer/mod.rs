use crate::{error::CompilerError, token::TokenKind};

use super::token::Token;
use located::{to_pos, to_span};
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while1},
    character::complete::{anychar, satisfy},
    combinator::{consumed, eof as nom_eof, map, map_res, success, value},
    error::{ErrorKind, FromExternalError, ParseError, VerboseError, VerboseErrorKind},
    multi::many0,
    Err, Finish, IResult, Parser,
};
use std::fmt::Write;
mod error;
mod located;
pub use located::LocatedStr;

trait ErrorBound<'a> = ParseError<LocatedStr<'a>> + FromExternalError<LocatedStr<'a>, Self>;

const IDENT_SYMBOLS: &str = "!$%&*+-/:<=>?^_~.";

fn identifier_like<'a, E: ErrorBound<'a>>(
    input: LocatedStr<'a>,
) -> IResult<LocatedStr<'a>, &'a str, E> {
    let (input, ident) =
        take_while1(|c: char| c.is_ascii_alphanumeric() || IDENT_SYMBOLS.contains(c))(input)?;
    Ok((input, ident.fragment()))
}

fn dot<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, ident) = identifier_like(input)?;
    if ident == "." {
        Ok((input, TokenKind::Dot))
    } else {
        Err(nom::Err::Error(E::from_error_kind(input, ErrorKind::Char)))
    }
}

fn number<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, sign) = alt((value(1, tag("+")), value(-1, tag("-")), success(1)))(input)?;
    let (input, ident) = identifier_like(input)?;
    if ident.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        let num = ident
            .parse::<i64>()
            .map_err(|_| Err::Failure(E::from_error_kind(input, ErrorKind::Digit)))?;
        Ok((input, TokenKind::Int(sign * num)))
    } else {
        Err(nom::Err::Error(E::from_error_kind(input, ErrorKind::Digit)))
    }
}

fn identifier<'a, E: ErrorBound<'a>>(
    input: LocatedStr<'a>,
) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, ident) = identifier_like(input)?;
    if !ident.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        Ok((input, TokenKind::Identifier(ident.to_string())))
    } else {
        Err(nom::Err::Error(E::from_error_kind(input, ErrorKind::Digit)))
    }
}

fn string<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, TokenKind::String(string.to_string())))
}

fn char<'a, E: ErrorBound<'a>>(input: LocatedStr<'a>) -> IResult<LocatedStr<'a>, TokenKind, E> {
    let (input, _) = tag("#\\")(input)?;
    let (input, ident) =
        alt((identifier_like, take(1usize).map(|s: LocatedStr| *s))).parse(input)?;
    if ident.chars().count() == 1 {
        Ok((input, TokenKind::Char(ident.chars().next().unwrap())))
    } else {
        match ident.to_lowercase().as_str() {
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
        string,
        char,
        // 順番に意味がある
        dot,
        number,
        identifier,
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
fn test_snapshot_lex() {
    use insta::assert_debug_snapshot;
    assert_debug_snapshot!(lex("(+ 1 2)"));
    assert_debug_snapshot!(lex(". abc"));
    assert_debug_snapshot!(lex("+1"));
    assert_debug_snapshot!(lex("+ 1"));
    assert_debug_snapshot!(lex("+a"));
    assert_debug_snapshot!(lex("#\\a"));
    assert_debug_snapshot!(lex("#\\\n"));
    assert_debug_snapshot!(lex("#\\newline"));
    assert_debug_snapshot!(lex("#\\nEwLine"));
}

#[test]
fn test_lex() {
    assert!(lex(".abc").is_err());
    assert!(lex("123abc").is_err());
    assert!(lex("+1a").is_err());
    assert!(lex("#\\").is_err());
    assert!(lex("#\\abc").is_err());
    assert!(lex("#\\.123").is_err());
}
