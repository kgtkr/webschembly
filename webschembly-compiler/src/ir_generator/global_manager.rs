use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ast, ir::*};

#[derive(Debug)]
pub struct GlobalManager {
    // GlobalIdのうち、ast::GlobalVarIdに対応するもの
    // 全てのGlobalIdがast::GlobalVarIdに対応するわけではない
    global_ids: FxHashMap<ast::GlobalVarId, GlobalId>,
    globals: FxHashMap<GlobalId, Global>,
    instantiated_globals: FxHashSet<GlobalId>,
}

impl Default for GlobalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalManager {
    pub fn new() -> Self {
        Self {
            global_ids: FxHashMap::default(),
            globals: FxHashMap::default(),
            instantiated_globals: FxHashSet::default(),
        }
    }

    pub fn gen_global(&mut self, typ: LocalType) -> Global {
        let id = GlobalId::from(self.globals.len());
        let global = Global {
            id,
            typ,
            linkage: GlobalLinkage::Export,
        };
        self.globals.insert(id, global);
        global
    }

    pub fn get_global_id(&self, id: ast::GlobalVarId) -> Option<GlobalId> {
        self.global_ids.get(&id).copied()
    }

    pub fn global(&mut self, id: ast::GlobalVarId) -> Global {
        let typ = LocalType::Type(Type::Obj);
        if let Some(&global_id) = self.global_ids.get(&id) {
            Global {
                id: global_id,
                typ,
                linkage: GlobalLinkage::Import,
            }
        } else {
            let global = self.gen_global(typ);
            self.global_ids.insert(id, global.id);

            global
        }
    }

    pub fn calc_module_globals(&mut self, global_ids: &[GlobalId]) -> FxHashMap<GlobalId, Global> {
        let mut globals = FxHashMap::default();
        for &global_id in global_ids {
            if self.instantiated_globals.contains(&global_id) {
                globals.insert(global_id, self.globals[&global_id].to_import());
            } else {
                globals.insert(global_id, self.globals[&global_id].to_export());
                self.instantiated_globals.insert(global_id);
            }
        }

        globals
    }
}
