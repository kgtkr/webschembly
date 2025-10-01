use crate::{VecMap, ir::Func};

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
