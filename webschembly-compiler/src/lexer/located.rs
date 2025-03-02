use nom_locate::LocatedSpan;

use crate::span::{Pos, Span};

pub type LocatedStr<'a> = LocatedSpan<&'a str>;

// トークンが複数行にまたがることはないという前提
pub fn to_span(located: &LocatedStr) -> Span {
    let start = to_pos(located);
    let end = Pos::new(
        start.line,
        start.column + located.fragment().chars().count(),
    );
    Span::new(start, end)
}

pub fn to_pos(located: &LocatedStr) -> Pos {
    Pos::new(
        located.location_line() as usize,
        located.get_utf8_column(),
    )
}
