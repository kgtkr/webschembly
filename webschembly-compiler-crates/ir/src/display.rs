use derive_more::{From, Into};

use super::meta::*;

#[derive(Debug, Clone)]
pub struct Display<'a, T> {
    pub value: T,
    pub meta: &'a Meta,
}

#[derive(Debug, Clone)]
pub struct DisplayInFunc<'a, T> {
    pub value: T,
    pub meta: MetaInFunc<'a>,
}

pub const DISPLAY_INDENT: &str = "  ";

#[derive(Debug, Clone, Copy, Default, Into, From)]
pub struct IndentLevel(pub usize);

impl IndentLevel {
    pub fn indent(&self) -> String {
        DISPLAY_INDENT.repeat(self.0)
    }

    pub fn increase(&self) -> Self {
        IndentLevel(self.0 + 1)
    }
}

impl std::fmt::Display for IndentLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.indent())
    }
}
