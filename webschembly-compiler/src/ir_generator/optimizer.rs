use super::analyzer::find_reachable_nodes;
use crate::{
    VecMap,
    ir::{BasicBlock, BasicBlockId},
};

pub fn remove_unreachable_bb(cfg: &mut VecMap<BasicBlockId, BasicBlock>, entry_id: BasicBlockId) {
    let reachable = find_reachable_nodes(cfg, entry_id);
    cfg.retain(|id, _| reachable.contains(&id));
}
