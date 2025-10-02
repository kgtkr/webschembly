use super::cfg_analyzer::find_reachable_nodes;
use crate::ir::Func;

pub fn remove_unreachable_bb(func: &mut Func) {
    let reachable = find_reachable_nodes(&func.bbs, func.bb_entry);
    func.bbs.retain(|id, _| reachable.contains(&id));
}
