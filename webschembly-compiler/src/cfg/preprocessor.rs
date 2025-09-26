use crate::{
    VecMap,
    cfg::find_reachable_nodes,
    ir::{BasicBlock, BasicBlockId},
};

pub fn preprocess_cfg(cfg: &mut VecMap<BasicBlockId, BasicBlock>, entry_id: BasicBlockId) {
    let reachable = find_reachable_nodes(&cfg, entry_id);
    cfg.retain(|id, _| reachable.contains(&id));
}
