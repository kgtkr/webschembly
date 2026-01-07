use rustc_hash::FxHashMap;
use vec_map::{VecMap, VecMapEq};
use webschembly_compiler_ir::*;

use crate::fxbihashmap::FxBiHashMap;
use crate::ir_generator::GlobalManager;

use super::index_flag::IndexFlag;

pub const BB_LAYOUT_MAX_SIZE: usize = 32;
pub const BB_LAYOUT_DEFAULT_INDEX: BBIndex = BBIndex(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BBIndex(pub usize);

#[derive(Debug)]
pub struct BBIndexManager {
    type_params_to_index: FxBiHashMap<VecMapEq<TypeParamId, ValType>, BBIndex>,
    index_to_global: FxHashMap<BBIndex, Global>,
}

impl BBIndexManager {
    pub fn new(global: Global) -> Self {
        let mut type_params_to_index = FxBiHashMap::default();
        let mut index_to_global = FxHashMap::default();
        type_params_to_index.insert(VecMapEq::from(VecMap::default()), BB_LAYOUT_DEFAULT_INDEX);
        index_to_global.insert(BB_LAYOUT_DEFAULT_INDEX, global);
        Self {
            type_params_to_index,
            index_to_global,
        }
    }

    pub fn idx(
        &mut self,
        type_params: &VecMap<TypeParamId, ValType>,
        global_manager: &mut GlobalManager,
    ) -> Option<(Global, BBIndex, IndexFlag)> {
        if let Some(&index) = self
            .type_params_to_index
            .get_by_left(VecMapEq::from_ref(type_params))
        {
            let global = *self.index_to_global.get(&index).unwrap();
            Some((global.to_import(), index, IndexFlag::ExistingInstance))
        } else if self.type_params_to_index.len() < BB_LAYOUT_MAX_SIZE {
            let index = BBIndex(self.type_params_to_index.len());
            self.type_params_to_index
                .insert(VecMapEq::from(type_params.clone()), index);
            let global = global_manager.gen_global(LocalType::FuncRef);
            self.index_to_global.insert(index, global);
            Some((global, index, IndexFlag::NewInstance))
        } else {
            None
        }
    }

    pub fn type_args(&self, index: BBIndex) -> (&VecMap<TypeParamId, ValType>, Global) {
        (
            self.type_params_to_index
                .get_by_right(&index)
                .unwrap()
                .as_inner(),
            self.index_to_global.get(&index).unwrap().to_import(),
        )
    }
}
