use nom::{
    error::{ErrorKind, ParseError},
    Err, IResult, InputIter, Parser, Slice,
};
use std::ops::RangeFrom;

pub fn satisfy_map_opt<I: Slice<RangeFrom<usize>> + InputIter, R, E: ParseError<I>>(
    f: impl Fn(I::Item) -> Option<R>,
) -> impl Parser<I, R, E> {
    move |input: I| match input.iter_elements().next() {
        Some(x) => match f(x) {
            Some(r) => Ok((input.slice(1..), r)),
            None => Err(Err::Error(E::from_error_kind(input, ErrorKind::MapOpt))),
        },
        None => Err(Err::Error(E::from_error_kind(input, ErrorKind::Eof))),
    }
}

pub fn satisfy<I: Slice<RangeFrom<usize>> + InputIter, F, E: ParseError<I>>(
    f: F,
) -> impl Parser<I, I::Item, E>
where
    F: Fn(I::Item) -> bool,
    I::Item: Copy,
{
    move |input: I| satisfy_map_opt(|x| if f(x) { Some(x) } else { None }).parse(input)
}

// パース結果がcondを満たすまでパースを続ける
// 例えば cond = |x| x == EOF なら、結果は[a, b, c, ..., EOF] になる
// もし EOF が現れなくてもmany0と同じようにパースに成功し、 [a, b, c, ..., z] を返す
pub fn many_until<I, O, E, F>(
    mut f: F,
    cond: impl Fn(&O) -> bool,
) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
    I: Clone,
    F: Parser<I, O, E>,
    E: ParseError<I>,
{
    move |mut i: I| {
        let mut acc = Vec::with_capacity(4);
        loop {
            match f.parse(i.clone()) {
                Err(Err::Error(_)) => return Ok((i, acc)),
                Err(e) => return Err(e),
                Ok((i1, o)) => {
                    i = i1;
                    let cond = cond(&o);
                    acc.push(o);
                    if cond {
                        return Ok((i, acc));
                    }
                }
            }
        }
    }
}
