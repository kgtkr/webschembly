use std::fmt;

use super::display::*;
use super::meta::*;
use derive_more::{From, Into};

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct FuncId(usize);

impl FuncId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, FuncId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, FuncId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "f{}", self.value.0)?;
        // TODO: add function name

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct LocalId(usize);

impl LocalId {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, LocalId> {
        DisplayInFunc { value: *self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, LocalId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "l{}", self.value.0)?;
        if let Some(meta) = self
            .meta
            .meta
            .local_metas
            .get(&(self.meta.func_id, self.value))
        {
            write!(f, "_{}", meta.name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct GlobalId(usize);

impl GlobalId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, GlobalId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, GlobalId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "g{}", self.value.0)?;
        if let Some(meta) = self.meta.global_metas.get(&self.value) {
            write!(f, "_{}", meta.name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct BasicBlockId(usize);

impl BasicBlockId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, BasicBlockId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, BasicBlockId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.value.0)?;
        // TODO: add basic block name
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct ModuleId(usize);

impl ModuleId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, ModuleId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, ModuleId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module{}", self.value.0)?;

        Ok(())
    }
}

// TODO: ここに置くべきじゃない
#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct TypeParamId(usize);
