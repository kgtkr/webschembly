use super::token::Token;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{anychar, satisfy, space1},
    combinator::{eof, map, map_res},
    multi::many0,
    sequence::tuple,
    IResult, Parser,
};

fn identifier(input: &str) -> IResult<&str, Token> {
    const SYMBOLS: &str = "!$%&*+-./:<=>?^_~";

    let (input, first) = satisfy(|c: char| c.is_ascii_alphabetic() || SYMBOLS.contains(c))(input)?;
    let (input, rest) =
        take_while(|c: char| c.is_ascii_alphanumeric() || SYMBOLS.contains(c))(input)?;
    Ok((input, Token::Identifier(format!("{}{}", first, rest))))
}

fn number(input: &str) -> IResult<&str, Token> {
    let (input, number) = map_res(take_while(|c: char| c.is_ascii_digit()), |s: &str| {
        s.parse::<i64>()
    })(input)?;
    Ok((input, Token::Number(number)))
}

fn string(input: &str) -> IResult<&str, Token> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, Token::String(string.to_string())))
}

fn character(input: &str) -> IResult<&str, Token> {
    let (input, _) = tag("#\\")(input)?;
    let (input, character) = anychar(input)?;
    Ok((input, Token::Character(character)))
}

fn token(input: &str) -> IResult<&str, Token> {
    let (input, token) = alt((
        identifier,
        tag("(").map(|_| Token::OpenParen),
        tag(")").map(|_| Token::CloseParen),
        number,
        string,
        tag("#t").map(|_| Token::Boolean(true)),
        tag("#f").map(|_| Token::Boolean(false)),
        character,
        tag("'").map(|_| Token::Quote),
    ))
    .parse(input)?;
    Ok((input, token))
}

fn space(input: &str) -> IResult<&str, ()> {
    map(space1, |_| ()).parse(input)
}

fn line_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag(";")(input)?;
    let (input, _) = take_while(|c: char| c != '\n')(input)?;
    Ok((input, ()))
}

fn ignore(input: &str) -> IResult<&str, ()> {
    let (input, _) = many0(alt((space, line_comment)))(input)?;
    Ok((input, ()))
}

fn token_and_ignore(input: &str) -> IResult<&str, Token> {
    let (input, token) = token(input)?;
    let (input, _) = ignore(input)?;
    Ok((input, token))
}

pub fn tokens(input: &str) -> IResult<&str, Vec<Token>> {
    let (input, _) = ignore(input)?;
    let (input, tokens) = many0(token_and_ignore)(input)?;
    let (input, _) = eof(input)?;
    Ok((input, tokens))
}
