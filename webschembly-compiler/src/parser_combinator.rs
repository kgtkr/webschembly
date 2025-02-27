use nom::{
    error::{ErrorKind, ParseError},
    Err, IResult, Parser,
};

pub fn satisfy_map_opt<'a, T: 'a, R, E: ParseError<&'a [T]>>(
    f: impl Fn(&T) -> Option<R>,
) -> impl Parser<&'a [T], R, E> {
    move |input: &'a [T]| match input.split_first() {
        Some((x, xs)) => match f(x) {
            Some(r) => Ok((xs, r)),
            None => Err(Err::Error(E::from_error_kind(input, ErrorKind::MapOpt))),
        },
        None => Err(Err::Error(E::from_error_kind(input, ErrorKind::Eof))),
    }
}

pub fn satisfy<'a, T: 'a, F, E: ParseError<&'a [T]>>(f: F) -> impl Parser<&'a [T], (), E>
where
    F: Fn(&T) -> bool,
{
    move |input: &'a [T]| satisfy_map_opt(|x| if f(x) { Some(()) } else { None }).parse(input)
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
