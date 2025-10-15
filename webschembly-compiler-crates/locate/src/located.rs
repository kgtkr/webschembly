use crate::Span;

#[derive(Debug, Clone)]
pub struct Located<T> {
    pub value: T,
    pub span: Span,
}

pub type L<T> = Located<T>;

pub trait LocatedValue: Sized {
    fn with_span(self, span: Span) -> L<Self> {
        L { value: self, span }
    }
}

impl<T> LocatedValue for T {}
