use nom::{
    error::{ErrorKind, ParseError},
    Err, Parser,
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
