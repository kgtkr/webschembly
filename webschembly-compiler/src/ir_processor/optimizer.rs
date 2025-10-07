use super::cfg_analyzer::find_reachable_nodes;
use crate::{VecMap, ir::Func};

// SSA関係なく可能な最適化関数の一覧

pub fn remove_unreachable_bb(func: &mut Func) {
    let reachable = find_reachable_nodes(&func.bbs, func.bb_entry);
    func.bbs.retain(|id, _| reachable.contains(&id));
}

pub fn remove_unused_local(func: &mut Func) {
    let mut local_used = VecMap::new();
    for local_id in func.locals.keys() {
        local_used.insert(local_id, false);
    }
    for &local_id in func.args.iter() {
        local_used[local_id] = true;
    }
    for bb in func.bbs.values() {
        for (&local, _) in bb.local_usages() {
            local_used[local] = true;
        }
    }
    func.locals.retain(|local_id, _| local_used[local_id]);
}
