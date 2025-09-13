use rustc_hash::FxHashMap;

use crate::{ast, ir::*};
use typed_index_collections::TiVec;

#[derive(Debug)]
pub struct IrGenerator {
    modules: TiVec<ModuleId, Option<Module>>,
    global_count: usize,
    // GlobalIdのうち、ast::GlobalVarIdに対応するもの
    // 全てのGlobalIdがast::GlobalVarIdに対応するわけではない
    global_ids: FxHashMap<ast::GlobalVarId, GlobalId>,
}

impl Default for IrGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl IrGenerator {
    pub fn new() -> Self {
        Self {
            modules: TiVec::new(),
            global_count: 0,
            global_ids: FxHashMap::default(),
        }
    }

    pub fn register_module(&mut self, module: Module) -> ModuleId {
        self.modules.push_and_get_key(Some(module))
    }

    pub fn alloc_module_id(&mut self) -> ModuleId {
        self.modules.push_and_get_key(None)
    }

    pub fn set_module(&mut self, id: ModuleId, module: Module) {
        assert!(
            self.modules[id].is_none(),
            "Module with id {:?} already exists",
            id
        );
        self.modules[id] = Some(module);
    }

    pub fn get_module(&self, id: ModuleId) -> &Module {
        self.modules[id].as_ref().unwrap()
    }

    pub fn gen_global_id(&mut self) -> GlobalId {
        let id = GlobalId::from(self.global_count);
        self.global_count += 1;
        id
    }

    pub fn global_id(&mut self, id: ast::GlobalVarId) -> GlobalId {
        if let Some(&global_id) = self.global_ids.get(&id) {
            global_id
        } else {
            let global_id = self.gen_global_id();
            self.global_ids.insert(id, global_id);

            global_id
        }
    }
}
