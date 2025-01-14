use super::token::Token;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::satisfy,
    combinator::{eof, map, map_res},
    multi::many0,
    IResult, Parser,
};

fn identifier(input: &str) -> IResult<&str, Token> {
    const SYMBOLS: &str = "!$%&*+-./:<=>?^_~";

    let (input, first) = satisfy(|c: char| c.is_ascii_alphabetic() || SYMBOLS.contains(c))(input)?;
    let (input, rest) =
        take_while(|c: char| c.is_ascii_alphanumeric() || SYMBOLS.contains(c))(input)?;
    Ok((input, Token::Identifier(format!("{}{}", first, rest))))
}

fn int(input: &str) -> IResult<&str, Token> {
    let (input, int) = map_res(take_while(|c: char| c.is_ascii_digit()), |s: &str| {
        s.parse::<i32>()
    })(input)?;
    Ok((input, Token::Int(int)))
}

fn string(input: &str) -> IResult<&str, Token> {
    let (input, _) = tag("\"")(input)?;
    let (input, string) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, Token::String(string.to_string())))
}
fn token(input: &str) -> IResult<&str, Token> {
    let (input, token) = alt((
        identifier,
        tag("(").map(|_| Token::OpenParen),
        tag(")").map(|_| Token::CloseParen),
        int,
        string,
        tag("#t").map(|_| Token::Bool(true)),
        tag("#f").map(|_| Token::Bool(false)),
        tag("'").map(|_| Token::Quote),
        tag(".").map(|_| Token::Dot),
    ))
    .parse(input)?;
    Ok((input, token))
}

fn space(input: &str) -> IResult<&str, ()> {
    map(take_while1(|c: char| c.is_ascii_whitespace()), |_| ()).parse(input)
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
