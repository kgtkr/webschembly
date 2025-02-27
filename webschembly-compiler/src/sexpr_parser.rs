use crate::sexpr::{Cons, SExprKind};
use crate::token::TokenKind;

use super::sexpr::SExpr;
use super::token::Token;

use super::parser_combinator::{satisfy, satisfy_map_opt};
use nom::combinator::opt;
use nom::sequence::preceded;
use nom::{branch::alt, multi::many0, IResult, Parser};

type Tokens<'a> = &'a [Token];

fn bool(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Bool(b) => Some(SExpr {
            kind: SExprKind::Bool(*b),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn int(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Int(i) => Some(SExpr {
            kind: SExprKind::Int(*i),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn string(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::String(s) => Some(SExpr {
            kind: SExprKind::String(s.clone()),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn symbol(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Identifier(s) => Some(SExpr {
            kind: SExprKind::Symbol(s.clone()),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn char(input: Tokens) -> IResult<Tokens, SExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Char(c) => Some(SExpr {
            kind: SExprKind::Char(*c),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

/*
再帰定義のほうがposの取得などは楽だが、スタックオーバーフローのリスクがある

fn list(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::OpenParen).parse(input)?;
    let (input, cons) = list_rec(input)?;
    Ok((input, cons))
}

fn list_rec(input: Tokens) -> IResult<Tokens, SExpr> {
    alt((
        satisfy(|t: &Token| t.kind == TokenKind::CloseParen).map(|_| SExpr::Nil),
        tuple((
            satisfy(|t: &Token| t.kind == TokenKind::Dot),
            sexpr,
            satisfy(|t: &Token| t.kind == TokenKind::CloseParen),
        ))
        .map(|(_, sexpr, _)| sexpr),
        tuple((sexpr, list_rec)).map(|(sexpr, cdr)| SExpr::Cons(Box::new(Cons::new(sexpr, cdr)))),
    ))
    .parse(input)
}
*/

fn list(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, _) = satisfy(|t: &Token| t.kind == TokenKind::OpenParen).parse(input)?;
    let (input, elements) = many0(sexpr)(input)?;
    let (input, tail) = opt(preceded(
        satisfy(|t: &Token| t.kind == TokenKind::Dot),
        sexpr,
    ))(input)?;
    let (input, close_token) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;

    let tail = tail.unwrap_or_else(|| SExpr {
        kind: SExprKind::Nil,
        span: close_token.span,
    });

    let list = elements.into_iter().rfold(tail, |cdr, car| {
        let span = car.span.merge(cdr.span);
        SExpr {
            kind: SExprKind::Cons(Box::new(Cons::new(car, cdr))),
            span,
        }
    });

    Ok((input, list))
}

fn quote(input: Tokens) -> IResult<Tokens, SExpr> {
    let (input, quote) = satisfy(|t: &Token| t.kind == TokenKind::Quote).parse(input)?;
    let (input, expr) = sexpr(input)?;
    let span = quote.span.merge(expr.span);

    Ok((
        input,
        list![
            SExpr {
                kind: SExprKind::Symbol("quote".to_string()),
                span: quote.span,
            } => span,
            expr => span,
            => span
        ],
    ))
}

fn sexpr(input: Tokens) -> IResult<Tokens, SExpr> {
    alt((bool, int, string, symbol, char, list, quote)).parse(input)
}

fn sexprs(input: Tokens) -> IResult<Tokens, Vec<SExpr>> {
    let (input, sexprs) = many0(sexpr).parse(input)?;
    Ok((input, sexprs))
}

pub fn parse(input: Tokens) -> Result<Vec<SExpr>, nom::Err<nom::error::Error<Tokens>>> {
    let (input, sexprs) = sexprs(input)?;
    let (_, _) = satisfy(|t: &Token| t.kind == TokenKind::Eof).parse(input)?;
    Ok(sexprs)
}
