use rustc_hash::FxHashSet;
use webschembly_compiler_ir::*;

use crate::fxbihashmap::FxBiHashMap;

use super::index_flag::IndexFlag;

pub const CLOSURE_LAYOUT_MAX_SIZE: usize = 32;
pub const CLOSURE_LAYOUT_DEFAULT_INDEX: ClosureIndex = ClosureIndex(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClosureIndex(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClosureArgs {
    Specified(Vec<Type>),
    Variadic,
}

#[derive(Debug)]
pub struct ClosureGlobalLayout {
    args_to_index: FxBiHashMap<ClosureArgs, ClosureIndex>,
    instantiated_idx: FxHashSet<ClosureIndex>,
}

impl Default for ClosureGlobalLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl ClosureGlobalLayout {
    pub fn new() -> Self {
        let mut args_to_index = FxBiHashMap::default();
        args_to_index.insert(ClosureArgs::Variadic, CLOSURE_LAYOUT_DEFAULT_INDEX);
        Self {
            args_to_index,
            instantiated_idx: FxHashSet::default(),
        }
    }

    pub fn idx(&mut self, args: &ClosureArgs) -> Option<(ClosureIndex, IndexFlag)> {
        // TODO: argsの長さに上限を設定
        let index = if let Some(&index) = self.args_to_index.get_by_left(args) {
            index
        } else if self.args_to_index.len() < CLOSURE_LAYOUT_MAX_SIZE {
            let index = ClosureIndex(self.args_to_index.len());
            self.args_to_index.insert(args.clone(), index);
            index
        } else {
            return None;
        };
        let flag = if self.instantiated_idx.insert(index) {
            IndexFlag::NewInstance
        } else {
            IndexFlag::ExistingInstance
        };
        Some((index, flag))
    }

    pub fn arg_types(&self, index: ClosureIndex) -> &ClosureArgs {
        self.args_to_index.get_by_right(&index).unwrap()
    }
}
