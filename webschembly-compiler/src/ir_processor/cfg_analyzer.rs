use rustc_hash::{FxHashMap, FxHashSet};
use vec_map::VecMap;
use webschembly_compiler_ir::*;

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

pub fn calc_rev_doms(
    doms: &FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>>,
) -> FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> {
    let mut rev_doms: FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> = FxHashMap::default();

    for (&node, dominated_nodes) in doms {
        for &dominated in dominated_nodes {
            rev_doms.entry(dominated).or_default().insert(node);
        }
    }

    rev_doms
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

pub fn has_critical_edges(cfg: &VecMap<BasicBlockId, BasicBlock>) -> bool {
    let mut pred_counts: FxHashMap<BasicBlockId, usize> = FxHashMap::default();
    let mut succ_counts: FxHashMap<BasicBlockId, usize> = FxHashMap::default();

    for (bb_id, bb) in cfg.iter() {
        let mut succ_count = 0;
        for succ_id in bb.next.successors() {
            *pred_counts.entry(succ_id).or_insert(0) += 1;
            succ_count += 1;
        }
        succ_counts.insert(bb_id, succ_count);
    }

    for (pred_id, pred_bb) in cfg.iter() {
        let pred_succ_count = *succ_counts.get(&pred_id).unwrap_or(&0);

        if pred_succ_count <= 1 {
            continue;
        }

        for succ_id in pred_bb.next.successors() {
            let succ_pred_count = *pred_counts.get(&succ_id).unwrap_or(&0);

            if succ_pred_count > 1 {
                return true;
            }
        }
    }

    false
}

pub fn calc_dominance_frontiers_from_tree(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    dom_tree: &DomTreeNode,
    predecessors: &FxHashMap<BasicBlockId, Vec<BasicBlockId>>,
) -> FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> {
    let mut dominance_frontiers: FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>> =
        FxHashMap::default();

    for id in cfg.keys() {
        dominance_frontiers.insert(id, FxHashSet::default());
    }

    calc_dominance_frontiers_recursive(dom_tree, cfg, predecessors, &mut dominance_frontiers);
    dominance_frontiers
}

fn calc_dominance_frontiers_recursive(
    node: &DomTreeNode,
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    predecessors: &FxHashMap<BasicBlockId, Vec<BasicBlockId>>,
    dominance_frontiers: &mut FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>>,
) {
    let mut df = FxHashSet::default();

    for child in &node.children {
        calc_dominance_frontiers_recursive(child, cfg, predecessors, dominance_frontiers);

        if let Some(child_df) = dominance_frontiers.get(&child.id) {
            for &frontier_node in child_df {
                if !dominates_in_tree(node, frontier_node) {
                    df.insert(frontier_node);
                }
            }
        }
    }

    if let Some(block) = cfg.get(node.id) {
        for successor in block.next.successors() {
            if !dominates_in_tree(node, successor) {
                df.insert(successor);
            }
        }
    }

    dominance_frontiers.insert(node.id, df);
}

fn dominates_in_tree(node: &DomTreeNode, target: BasicBlockId) -> bool {
    if node.id == target {
        return true;
    }

    for child in &node.children {
        if dominates_in_tree(child, target) {
            return true;
        }
    }

    false
}
