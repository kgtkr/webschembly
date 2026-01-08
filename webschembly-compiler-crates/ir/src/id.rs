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
pub struct JitModuleId(usize);

impl JitModuleId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, JitModuleId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, JitModuleId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "jit_module{}", self.value.0)?;

        Ok(())
    }
}

// TODO: ここに置くべきじゃない
#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct TypeParamId(usize);

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct JitFuncId(usize);

impl JitFuncId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, JitFuncId> {
        Display { value: *self, meta }
    }
}

impl From<FuncId> for JitFuncId {
    fn from(func_id: FuncId) -> Self {
        JitFuncId(func_id.0)
    }
}

impl From<JitFuncId> for FuncId {
    fn from(jit_func_id: JitFuncId) -> Self {
        FuncId(jit_func_id.0)
    }
}

impl fmt::Display for Display<'_, JitFuncId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "jit_func{}", self.value.0)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct JitBasicBlockId(usize);
impl JitBasicBlockId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, JitBasicBlockId> {
        Display { value: *self, meta }
    }
}

impl From<BasicBlockId> for JitBasicBlockId {
    fn from(bb_id: BasicBlockId) -> Self {
        JitBasicBlockId(bb_id.0)
    }
}

impl From<JitBasicBlockId> for BasicBlockId {
    fn from(jit_bb_id: JitBasicBlockId) -> Self {
        BasicBlockId(jit_bb_id.0)
    }
}

impl fmt::Display for Display<'_, JitBasicBlockId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "jit_bb{}", self.value.0)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct ClosureEnvIndex(pub usize);

impl ClosureEnvIndex {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, ClosureEnvIndex> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, ClosureEnvIndex> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "closure_env{}", self.value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct ClosureArgIndex(pub usize);

impl ClosureArgIndex {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, ClosureArgIndex> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, ClosureArgIndex> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "closure_arg{}", self.value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct BBIndex(pub usize);

impl BBIndex {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, BBIndex> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, BBIndex> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb_index{}", self.value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantClosure {
    pub func_id: JitFuncId,
    pub env_index: ClosureEnvIndex,
}

impl ConstantClosure {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, ConstantClosure> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, ConstantClosure> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "constant_closure({}, {})",
            self.value.func_id.display(self.meta),
            self.value.env_index.display(self.meta)
        )
    }
}
