use super::id::*;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
}
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub local_metas: FxHashMap<(FuncId, LocalId), VarMeta>,
    pub global_metas: FxHashMap<GlobalId, VarMeta>,
}

impl Meta {
    pub fn in_func<'a>(&'a self, func_id: FuncId) -> MetaInFunc<'a> {
        MetaInFunc {
            func_id,
            meta: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MetaInFunc<'a> {
    pub func_id: FuncId,
    pub meta: &'a Meta,
}
