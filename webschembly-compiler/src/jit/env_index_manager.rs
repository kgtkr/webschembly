use rustc_hash::FxHashMap;
use vec_map::{VecMap, VecMapEq};
use webschembly_compiler_ir::*;

use crate::fxbihashmap::FxBiHashMap;
use crate::ir_generator::GlobalManager;

use super::index_flag::IndexFlag;

pub const ENV_LAYOUT_MAX_SIZE: usize = 32;
pub const ENV_LAYOUT_DEFAULT_INDEX: EnvIndex = EnvIndex(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvIndex(pub usize);

#[derive(Debug)]
pub struct EnvIndexManager {
    env_types_to_index: FxBiHashMap<VecMapEq<usize, ValType>, EnvIndex>,
    index_to_table_global: FxHashMap<EnvIndex, Global>,
}

impl Default for EnvIndexManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvIndexManager {
    pub fn new() -> Self {
        let mut env_types_to_index = FxBiHashMap::default();
        env_types_to_index.insert(VecMapEq::from(VecMap::default()), ENV_LAYOUT_DEFAULT_INDEX);

        Self {
            env_types_to_index,
            index_to_table_global: FxHashMap::default(),
        }
    }

    pub fn idx(
        &mut self,
        env_types: &VecMap<usize, ValType>,
        global_manager: &mut GlobalManager,
    ) -> Option<(Option<Global>, EnvIndex, IndexFlag)> {
        if let Some(&index) = self
            .env_types_to_index
            .get_by_left(VecMapEq::from_ref(env_types))
        {
            let global = self.index_to_table_global.get(&index);
            debug_assert!(index == ENV_LAYOUT_DEFAULT_INDEX || global.is_some());
            Some((global.copied(), index, IndexFlag::ExistingInstance))
        } else if self.env_types_to_index.len() < ENV_LAYOUT_MAX_SIZE {
            let index = EnvIndex(self.env_types_to_index.len());
            self.env_types_to_index
                .insert(env_types.clone().into(), index);
            let global = global_manager.gen_global(LocalType::EntrypointTable);
            self.index_to_table_global.insert(index, global);
            Some((Some(global), index, IndexFlag::NewInstance))
        } else {
            None
        }
    }

    pub fn env_types(&self, index: EnvIndex) -> (&VecMap<usize, ValType>, Option<Global>) {
        debug_assert!(
            index == ENV_LAYOUT_DEFAULT_INDEX || self.index_to_table_global.contains_key(&index)
        );
        (
            self.env_types_to_index
                .get_by_right(&index)
                .unwrap()
                .as_inner(),
            self.index_to_table_global.get(&index).copied(),
        )
    }
}
