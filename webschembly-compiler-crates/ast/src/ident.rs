use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Mark(pub usize);

impl Mark {
    pub const ROOT: Self = Mark(0);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    pub name: String,
    pub mark: Mark,
}

impl Ident {
    pub fn new(name: impl Into<String>, mark: Mark) -> Self {
        Self {
            name: name.into(),
            mark,
        }
    }

    pub fn root(name: impl Into<String>) -> Self {
        Self::new(name, Mark::ROOT)
    }
}

impl Display for Ident {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.mark == Mark::ROOT {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}#{}", self.name, self.mark.0)
        }
    }
}
