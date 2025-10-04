use rustc_hash::FxHashMap;

use crate::{ast, ir::*};

#[derive(Debug)]
pub struct GlobalManager {
    global_count: usize,
    // GlobalIdのうち、ast::GlobalVarIdに対応するもの
    // 全てのGlobalIdがast::GlobalVarIdに対応するわけではない
    global_ids: FxHashMap<ast::GlobalVarId, GlobalId>,
}

impl Default for GlobalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalManager {
    pub fn new() -> Self {
        Self {
            global_count: 0,
            global_ids: FxHashMap::default(),
        }
    }

    pub fn gen_global(&mut self, typ: LocalType) -> Global {
        let id = GlobalId::from(self.global_count);
        self.global_count += 1;
        Global {
            id,
            typ,
            linkage: GlobalLinkage::Export,
        }
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
}
