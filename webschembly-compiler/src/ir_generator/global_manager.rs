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
