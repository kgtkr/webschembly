use rustc_hash::{FxHashMap, FxHashSet};

use vec_map::VecMap;
use webschembly_compiler_ir::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefUse {
    pub defs: FxHashSet<LocalId>,
    pub uses: FxHashSet<LocalId>,
}

pub fn calc_def_use(cfg: &VecMap<BasicBlockId, BasicBlock>) -> FxHashMap<BasicBlockId, DefUse> {
    let mut def_use_map = FxHashMap::default();

    for (block_id, block) in cfg.iter() {
        let mut defs = FxHashSet::default();
        let mut uses = FxHashSet::default();

        for (local_id, flag) in block.local_usages() {
            match flag {
                LocalFlag::Defined => {
                    defs.insert(*local_id);
                }
                LocalFlag::Used(used_flag) => {
                    match used_flag {
                        // Phiノードで使われている変数は、ブロック内で定義されていなくてもuseに含めない
                        LocalUsedFlag::Phi(_) => {}
                        LocalUsedFlag::NonPhi => {
                            if !defs.contains(local_id) {
                                uses.insert(*local_id);
                            }
                        }
                    }
                }
            }
        }

        def_use_map.insert(block_id, DefUse { defs, uses });
    }

    def_use_map
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessInfo {
    pub live_in: FxHashMap<BasicBlockId, FxHashSet<LocalId>>,
    pub live_out: FxHashMap<BasicBlockId, FxHashSet<LocalId>>,
}

pub fn analyze_liveness(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    def_use: &FxHashMap<BasicBlockId, DefUse>,
    rpo: &FxHashMap<BasicBlockId, usize>,
) -> LivenessInfo {
    let mut live_in: FxHashMap<BasicBlockId, FxHashSet<LocalId>> = FxHashMap::default();
    let mut live_out: FxHashMap<BasicBlockId, FxHashSet<LocalId>> = FxHashMap::default();

    // 空集合で初期化
    for (block_id, _) in cfg.iter() {
        live_in.insert(block_id, FxHashSet::default());
        live_out.insert(block_id, FxHashSet::default());
    }

    // 逆RPO順で計算すると収束が速い
    let mut rpo_nodes = cfg.keys().collect::<Vec<_>>();
    rpo_nodes.sort_by_key(|id| std::cmp::Reverse(rpo.get(id).unwrap()));

    let mut changed = true;
    while changed {
        changed = false;

        for &block_id in &rpo_nodes {
            let block = cfg.get(block_id).unwrap();
            let def_use_info = &def_use[&block_id];

            // live_in[B] = uses[B] ∪ (live_out[B] - defs[B])
            let mut new_live_in = def_use_info.uses.clone();
            let live_out_minus_defs = live_out[&block_id]
                .difference(&def_use_info.defs)
                .cloned()
                .collect::<FxHashSet<_>>();
            new_live_in.extend(live_out_minus_defs);

            // live_out[B] = ∪ live_in[S]
            let mut new_live_out = FxHashSet::default();
            for successor in block.terminator().successors() {
                new_live_out.extend(&live_in[&successor]);
                let successor_block = &cfg[successor];
                for (id, flag) in successor_block.local_usages() {
                    if let LocalFlag::Used(LocalUsedFlag::Phi(phi_bb_id)) = flag
                        && phi_bb_id == block_id
                    {
                        new_live_out.insert(*id);
                    }
                }
            }

            if new_live_in != live_in[&block_id] || new_live_out != live_out[&block_id] {
                changed = true;
                live_in.insert(block_id, new_live_in);
                live_out.insert(block_id, new_live_out);
            }
        }
    }

    LivenessInfo { live_in, live_out }
}
