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
