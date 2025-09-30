use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    VecMap,
    ir::{BasicBlock, BasicBlockId, LocalFlag, LocalId},
};

pub fn calc_predecessors(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
) -> FxHashMap<BasicBlockId, Vec<BasicBlockId>> {
    let mut predecessors: FxHashMap<BasicBlockId, Vec<BasicBlockId>> = FxHashMap::default();
    for (id, block) in cfg.iter() {
        for successor in block.next.successors() {
            predecessors.entry(successor).or_default().push(id);
        }
    }
    predecessors
}

pub fn calc_doms(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    rpo: &FxHashMap<BasicBlockId, usize>,
    entry_id: BasicBlockId,
    predecessors: &FxHashMap<BasicBlockId, Vec<BasicBlockId>>,
) -> FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> {
    let all_nodes: FxHashSet<BasicBlockId> = cfg.keys().collect();
    let mut doms: FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> = FxHashMap::default();

    doms.insert(entry_id, [entry_id].iter().cloned().collect());
    for &id in &all_nodes {
        if id != entry_id {
            doms.insert(id, all_nodes.clone());
        }
    }

    let mut rpo_nodes: Vec<_> = cfg.keys().collect();
    rpo_nodes.sort_by_key(|id| rpo.get(id).expect("RPO must contain all nodes"));

    let mut changed = true;
    while changed {
        changed = false;
        for &id in &rpo_nodes {
            if id == entry_id {
                continue;
            }

            let empty = vec![];
            let preds = predecessors.get(&id).unwrap_or(&empty);
            if preds.is_empty() {
                continue;
            }

            // new_dom = {n} U (intersection of dom(p) for all p in predecessors)
            let mut new_dom = preds
                .iter()
                .map(|p| doms.get(p).unwrap())
                .cloned()
                .reduce(|acc, set| acc.intersection(&set).cloned().collect())
                .unwrap_or_default();
            new_dom.insert(id);

            if &new_dom != doms.get(&id).unwrap() {
                doms.insert(id, new_dom);
                changed = true;
            }
        }
    }

    doms
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomTreeNode {
    pub id: BasicBlockId,
    pub children: Vec<DomTreeNode>,
}

pub fn build_dom_tree(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    rpo: &FxHashMap<BasicBlockId, usize>,
    entry_id: BasicBlockId,
    doms: &FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>>,
) -> DomTreeNode {
    let mut idoms: FxHashMap<BasicBlockId, BasicBlockId> = FxHashMap::default();
    for id in cfg.keys() {
        if id == entry_id {
            continue;
        }

        // idomは、自分以外の支配ノードの中で最もRPO番号が大きいもの
        let idom = doms
            .get(&id)
            .unwrap()
            .iter()
            .filter(|&&d| d != id)
            .max_by_key(|&&d| rpo.get(&d).unwrap())
            .unwrap();
        idoms.insert(id, *idom);
    }

    // --- Step D: idom関係から木構造を構築 ---
    let mut children_map: FxHashMap<BasicBlockId, Vec<BasicBlockId>> = FxHashMap::default();
    for (child, parent) in idoms {
        children_map.entry(parent).or_default().push(child);
    }

    build_tree_recursive(entry_id, &children_map)
}

fn build_tree_recursive(
    id: BasicBlockId,
    children_map: &FxHashMap<BasicBlockId, Vec<BasicBlockId>>,
) -> DomTreeNode {
    let children = match children_map.get(&id) {
        Some(child_ids) => child_ids
            .iter()
            .map(|&child_id| build_tree_recursive(child_id, children_map))
            .collect(),
        None => Vec::new(),
    };
    DomTreeNode { id, children }
}

pub fn calculate_rpo(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    entry_id: BasicBlockId,
) -> FxHashMap<BasicBlockId, usize> {
    let mut visited = FxHashSet::default();
    let mut postorder = Vec::new();

    // DFSを行い、帰りがけ順でノードを記録する
    dfs_postorder(entry_id, cfg, &mut visited, &mut postorder);

    // 帰りがけ順 (postorder) を反転させたものが逆後順序 (RPO)
    postorder.reverse();

    // RPOの順序を元に、各IDに番号 (インデックス) をマッピングする
    postorder
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect()
}

fn dfs_postorder(
    current_id: BasicBlockId,
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    visited: &mut FxHashSet<BasicBlockId>,
    postorder: &mut Vec<BasicBlockId>,
) {
    visited.insert(current_id);
    let node = cfg.get(current_id).unwrap();

    for successor in node.next.successors() {
        if !visited.contains(&successor) {
            dfs_postorder(successor, cfg, visited, postorder);
        }
    }

    // すべての子孫の訪問が終わった後 (帰りがけ) に自身を追加
    postorder.push(current_id);
}

pub fn find_loop_headers(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    rpo: &FxHashMap<BasicBlockId, usize>,
) -> FxHashSet<BasicBlockId> {
    let mut headers = FxHashSet::default();
    for (source_id, block) in cfg.iter() {
        for target_id in block.next.successors() {
            let source_rpo = rpo.get(&source_id).unwrap();
            let target_rpo = rpo.get(&target_id).unwrap();

            // RPO番号が小さくなる方向へのジャンプ (後方エッジ) のターゲットがループヘッダ
            if source_rpo >= target_rpo {
                headers.insert(target_id);
            }
        }
    }
    headers
}

pub fn find_merge_nodes(
    rpo: &FxHashMap<BasicBlockId, usize>,
    predecessors: &FxHashMap<BasicBlockId, Vec<BasicBlockId>>,
) -> FxHashSet<BasicBlockId> {
    let mut merge_nodes = FxHashSet::default();
    for (&id, preds) in predecessors {
        // 先行ノードからのエッジが「前方エッジ」であるものの数を数える
        let forward_preds_count = preds
            .iter()
            .filter(|&&pred_id| rpo.get(&pred_id) < rpo.get(&id))
            .count();

        // 前方エッジが2つ以上あればマージノード
        if forward_preds_count >= 2 {
            merge_nodes.insert(id);
        }
    }
    merge_nodes
}

pub fn find_reachable_nodes(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    entry_id: BasicBlockId,
) -> FxHashSet<BasicBlockId> {
    let mut reachable = FxHashSet::default();
    let mut worklist = vec![entry_id];
    reachable.insert(entry_id);

    while let Some(id) = worklist.pop() {
        let node = cfg.get(id).unwrap();

        for successor in node.next.successors() {
            if reachable.insert(successor) {
                worklist.push(successor);
            }
        }
    }
    reachable
}

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
                LocalFlag::Used => {
                    if !defs.contains(local_id) {
                        uses.insert(*local_id);
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
            for successor in block.next.successors() {
                new_live_out.extend(&live_in[&successor]);
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
