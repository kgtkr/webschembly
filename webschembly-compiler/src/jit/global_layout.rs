use rustc_hash::{FxHashMap, FxHashSet};

use crate::fxbihashmap::FxBiHashMap;
use crate::ir_generator::GlobalManager;
use vec_map::{VecMap, VecMapEq};
use webschembly_compiler_ir::*;

pub const GLOBAL_LAYOUT_MAX_SIZE: usize = 32;
pub const GLOBAL_LAYOUT_DEFAULT_INDEX: usize = 0;

#[derive(Debug)]
pub struct BBIndexManager {
    type_params_to_index: FxBiHashMap<VecMapEq<TypeParamId, ValType>, usize>,
    index_to_global: FxHashMap<usize, Global>,
}

impl BBIndexManager {
    pub fn new(global: Global) -> Self {
        let mut type_params_to_index = FxBiHashMap::default();
        let mut index_to_global = FxHashMap::default();
        type_params_to_index.insert(
            VecMapEq::from(VecMap::default()),
            GLOBAL_LAYOUT_DEFAULT_INDEX,
        );
        index_to_global.insert(GLOBAL_LAYOUT_DEFAULT_INDEX, global);
        Self {
            type_params_to_index,
            index_to_global,
        }
    }

    pub fn idx(
        &mut self,
        type_params: &VecMap<TypeParamId, ValType>,
        global_manager: &mut GlobalManager,
    ) -> Option<(Global, usize, IndexFlag)> {
        if let Some(&index) = self
            .type_params_to_index
            .get_by_left(VecMapEq::from_ref(type_params))
        {
            let global = *self.index_to_global.get(&index).unwrap();
            Some((global.to_import(), index, IndexFlag::ExistingInstance))
        } else if self.type_params_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.type_params_to_index.len();
            self.type_params_to_index
                .insert(VecMapEq::from(type_params.clone()), index);
            let global = global_manager.gen_global(LocalType::FuncRef);
            self.index_to_global.insert(index, global);
            Some((global, index, IndexFlag::NewInstance))
        } else {
            None
        }
    }

    pub fn type_args(&self, index: usize) -> (&VecMap<TypeParamId, ValType>, Global) {
        (
            self.type_params_to_index
                .get_by_right(&index)
                .unwrap()
                .as_inner(),
            self.index_to_global.get(&index).unwrap().to_import(),
        )
    }
}

#[derive(Debug)]
pub struct EnvIndexManager {
    env_types_to_index: FxBiHashMap<VecMapEq<usize, ValType>, usize>,
    index_to_table_global: FxHashMap<usize, Global>,
}

impl EnvIndexManager {
    pub fn new() -> Self {
        let mut env_types_to_index = FxBiHashMap::default();
        env_types_to_index.insert(
            VecMapEq::from(VecMap::default()),
            GLOBAL_LAYOUT_DEFAULT_INDEX,
        );
        // TODO: index_to_table_globalのデフォルト値

        Self {
            env_types_to_index,
            index_to_table_global: FxHashMap::default(),
        }
    }

    pub fn idx(
        &mut self,
        env_types: &VecMap<usize, ValType>,
        global_manager: &mut GlobalManager,
    ) -> Option<(Global, usize, IndexFlag)> {
        if let Some(&index) = self
            .env_types_to_index
            .get_by_left(VecMapEq::from_ref(env_types))
        {
            let global = *self.index_to_table_global.get(&index).unwrap();
            Some((global.to_import(), index, IndexFlag::ExistingInstance))
        } else if self.env_types_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.env_types_to_index.len();
            self.env_types_to_index
                .insert(env_types.clone().into(), index);
            let global = global_manager.gen_global(LocalType::EntrypointTable);
            self.index_to_table_global.insert(index, global);
            Some((global, index, IndexFlag::NewInstance))
        } else {
            None
        }
    }

    pub fn env_types(&self, index: usize) -> (&VecMap<usize, ValType>, Global) {
        (
            self.env_types_to_index
                .get_by_right(&index)
                .unwrap()
                .as_inner(),
            self.index_to_table_global.get(&index).unwrap().to_import(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFlag {
    NewInstance,
    ExistingInstance,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClosureArgs {
    Specified(Vec<Type>),
    Variadic,
}

#[derive(Debug)]
pub struct ClosureGlobalLayout {
    args_to_index: FxBiHashMap<ClosureArgs, usize>,
    instantiated_idx: FxHashSet<usize>,
}

impl Default for ClosureGlobalLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl ClosureGlobalLayout {
    pub fn new() -> Self {
        let mut args_to_index = FxBiHashMap::default();
        args_to_index.insert(ClosureArgs::Variadic, GLOBAL_LAYOUT_DEFAULT_INDEX);
        Self {
            args_to_index,
            instantiated_idx: FxHashSet::default(),
        }
    }

    pub fn idx(&mut self, args: &ClosureArgs) -> Option<(usize, IndexFlag)> {
        // TODO: argsの長さに上限を設定
        let index = if let Some(&index) = self.args_to_index.get_by_left(args) {
            index
        } else if self.args_to_index.len() < GLOBAL_LAYOUT_MAX_SIZE {
            let index = self.args_to_index.len();
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

    pub fn arg_types(&self, index: usize) -> &ClosureArgs {
        self.args_to_index.get_by_right(&index).unwrap()
    }
}
