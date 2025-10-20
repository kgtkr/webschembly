use crate::token::TokenKind;

use super::token::Token;
use webschembly_compiler_sexpr::{Cons, LSExpr, SExpr, SUVectorKind, list};

use super::parser_combinator::{satisfy, satisfy_map_opt};
use crate::tokens::Tokens;
use nom::combinator::opt;
use nom::sequence::preceded;
use nom::{IResult, Parser, branch::alt, multi::many0};

fn bool(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Bool(b) => Some(LSExpr {
            value: SExpr::Bool(*b),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn int(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Int(i) => Some(LSExpr {
            value: SExpr::Int(*i),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn float(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Float(f) => Some(LSExpr {
            value: SExpr::Float(*f),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn nan(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::NaN => Some(LSExpr {
            value: SExpr::NaN,
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn string(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::String(s) => Some(LSExpr {
            value: SExpr::String(s.clone()),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn symbol(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Identifier(s) => Some(LSExpr {
            value: SExpr::Symbol(s.clone()),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn char(input: Tokens) -> IResult<Tokens, LSExpr> {
    satisfy_map_opt(|t: &Token| match &t.kind {
        TokenKind::Char(c) => Some(LSExpr {
            value: SExpr::Char(*c),
            span: t.span,
        }),
        _ => None,
    })
    .parse(input)
}

fn vector(input: Tokens) -> IResult<Tokens, LSExpr> {
    let (input, open_token) =
        satisfy(|t: &Token| t.kind == TokenKind::VectorOpenParen).parse(input)?;
    let (input, elements) = many0(sexpr)(input)?;
    let (input, close_token) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;

    let vector = LSExpr {
        value: SExpr::Vector(elements),
        span: open_token.span.merge(close_token.span),
    };

    Ok((input, vector))
}

fn uvector(input: Tokens) -> IResult<Tokens, LSExpr> {
    let (input, (open_token, kind)) = alt((
        satisfy(|t: &Token| t.kind == TokenKind::UVectorS64OpenParen)
            .map(|open_token| (open_token, SUVectorKind::S64)),
        satisfy(|t: &Token| t.kind == TokenKind::UVectorF64OpenParen)
            .map(|open_token| (open_token, SUVectorKind::F64)),
    ))
    .parse(input)?;
    let (input, elements) = many0(sexpr)(input)?;
    let (input, close_token) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;

    let uvector = LSExpr {
        value: SExpr::UVector(kind, elements),
        span: open_token.span.merge(close_token.span),
    };

    Ok((input, uvector))
}

fn list(input: Tokens) -> IResult<Tokens, LSExpr> {
    let (input, open_token) = satisfy(|t: &Token| t.kind == TokenKind::OpenParen).parse(input)?;
    let (input, elements) = many0(sexpr)(input)?;
    let (input, tail) = opt(preceded(
        satisfy(|t: &Token| t.kind == TokenKind::Dot),
        sexpr,
    ))(input)?;
    let (input, close_token) = satisfy(|t: &Token| t.kind == TokenKind::CloseParen).parse(input)?;
    let is_dotted = tail.is_some();
    let elements_is_empty = elements.is_empty();
    let tail = tail.unwrap_or(LSExpr {
        value: SExpr::Nil,
        span: close_token.span,
    });

    let list = elements.into_iter().rfold(tail, |cdr, car| {
        let span = car.span.merge(cdr.span);
        LSExpr {
            value: SExpr::Cons(Box::new(Cons::new(car, cdr))),
            span,
        }
    });

    Ok((
        input,
        if is_dotted && elements_is_empty {
            /*
            (. 1) のようにdotted listだがドットの前に要素がない場合はspanを拡張しない
            そもそもこのようなリストはgoshだとエラーだが、エラーにする理由はあまりないので認めることにする

            (. sexpr) は sexpr と同じ意味で、他言語の (expr) と似たようなものであるためspanは拡張するべきではないという理由
            */

            list
        } else {
            // 一番外側のCons / Nilのspanを開き括弧から閉じ括弧までに拡張する
            let mut list = list;
            list.span = open_token.span.merge(close_token.span);
            list
        },
    ))
}

fn quote(input: Tokens) -> IResult<Tokens, LSExpr> {
    let (input, quote) = satisfy(|t: &Token| t.kind == TokenKind::Quote).parse(input)?;
    let (input, expr) = sexpr(input)?;
    let span = quote.span.merge(expr.span);

    Ok((
        input,
        list![
            LSExpr {
                value: SExpr::Symbol("quote".to_string()),
                span: quote.span,
            } => span,
            expr => span,
            => span
        ],
    ))
}

fn sexpr(input: Tokens) -> IResult<Tokens, LSExpr> {
    alt((
        bool, int, float, nan, string, symbol, char, list, vector, uvector, quote,
    ))
    .parse(input)
}

fn sexprs(input: Tokens) -> IResult<Tokens, Vec<LSExpr>> {
    let (input, sexprs) = many0(sexpr).parse(input)?;
    Ok((input, sexprs))
}

pub fn parse<'a>(
    input: &'a [Token],
) -> Result<Vec<LSExpr>, nom::Err<nom::error::Error<Tokens<'a>>>> {
    let input = Tokens::new(input);
    let (input, sexprs) = sexprs(input)?;
    let (_, _) = satisfy(|t: &Token| t.kind == TokenKind::Eof).parse(input)?;
    Ok(sexprs)
}
