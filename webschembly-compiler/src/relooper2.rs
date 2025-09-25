// https://dl.acm.org/doi/abs/10.1145/3547621
// by gemini

use std::collections::{HashMap, HashSet};

// --- 識別子 ---
pub type LocalId = usize;
pub type BasicBlockId = usize;

// --- 入力 (Input) ---

#[derive(Debug, Clone)]
pub enum Terminator {
    If {
        condition: LocalId,
        then_block: BasicBlockId,
        else_block: BasicBlockId,
    },
    Jump(BasicBlockId),
    Return,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub terminator: Terminator,
}

pub type CFG = HashMap<BasicBlockId, BasicBlock>;

// --- 出力 (Output) ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Structured {
    Simple(BasicBlockId),
    If {
        condition: LocalId,
        then_branch: Vec<Structured>,
        else_branch: Vec<Structured>,
    },
    Block {
        body: Vec<Structured>,
    },
    Loop {
        body: Vec<Structured>,
    },
    Break(usize),
    Return,
}

// --- 事前解析で必要となるデータ構造 ---

// ドミネーターツリーのノード
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomTreeNode {
    pub id: BasicBlockId,
    pub children: Vec<DomTreeNode>,
}

// 論文の'Context'に相当。brのターゲットとなる構文のネストを表す
#[derive(Debug, Clone, Copy)]
enum ContainingSyntax {
    IfThenElse,
    LoopHeadedBy(BasicBlockId),
    BlockFollowedBy(BasicBlockId),
}

// --- メインの変換ロジック ---

struct Translator<'a> {
    cfg: &'a CFG,
    dom_tree: &'a DomTreeNode,
    rpo: &'a HashMap<BasicBlockId, usize>, // 逆後順序番号
    loop_headers: &'a HashSet<BasicBlockId>,
    merge_nodes: &'a HashSet<BasicBlockId>,
}

impl<'a> Translator<'a> {
    fn index(&self, target_id: BasicBlockId, context: &[ContainingSyntax]) -> Option<usize> {
        for (i, syntax) in context.iter().enumerate() {
            match syntax {
                ContainingSyntax::LoopHeadedBy(id) if *id == target_id => return Some(i),
                ContainingSyntax::BlockFollowedBy(id) if *id == target_id => return Some(i),
                _ => {}
            }
        }
        None // 本来は見つかるはず
    }

    /// 論文の doTree (line 1-7)
    fn do_tree(&self, tree_node: &DomTreeNode, context: &[ContainingSyntax]) -> Vec<Structured> {
        let x = tree_node.id;

        let mut merge_children: Vec<_> = tree_node
            .children
            .iter()
            .filter(|child| self.merge_nodes.contains(&child.id))
            .collect();
        merge_children.sort_by_key(|child| -(*self.rpo.get(&child.id).unwrap_or(&0) as isize));

        if self.loop_headers.contains(&x) {
            // ループヘッダの場合
            let mut new_context = context.to_vec();
            new_context.insert(0, ContainingSyntax::LoopHeadedBy(x));

            vec![Structured::Loop {
                body: self.node_within(tree_node, &merge_children, &new_context),
            }]
        } else {
            // ループヘッdaではない場合
            self.node_within(tree_node, &merge_children, context)
        }
    }

    /// 論文の nodeWithin (line 8-21)
    fn node_within(
        &self,
        tree_node: &DomTreeNode,
        merge_children: &[&DomTreeNode],
        context: &[ContainingSyntax],
    ) -> Vec<Structured> {
        if let Some((y_n_node, rest_ys)) = merge_children.split_first() {
            // [再帰ケース] マージノードの子が残っている場合
            let y_n = y_n_node.id;

            // コンテキストにBlockFollowedByを追加
            let mut new_context = context.to_vec();
            new_context.insert(0, ContainingSyntax::BlockFollowedBy(y_n));

            let mut result = Vec::new();
            result.push(Structured::Block {
                body: self.node_within(tree_node, rest_ys, &new_context),
            });
            result.extend(self.do_tree(y_n_node, context));
            result
        } else {
            // [ベースケース] マージノードの子をすべて処理した場合
            let x = tree_node.id;
            let block = self.cfg.get(&x).unwrap();

            let mut result = Vec::new();
            result.push(Structured::Simple(block.id));

            match &block.terminator {
                Terminator::Jump(target) => {
                    result.extend(self.do_branch(x, *target, context));
                }
                Terminator::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let mut new_context = context.to_vec();
                    new_context.insert(0, ContainingSyntax::IfThenElse);

                    let then_branch = self.do_branch(x, *then_block, &new_context);
                    let else_branch = self.do_branch(x, *else_block, &new_context);

                    result.push(Structured::If {
                        condition: *condition,
                        then_branch,
                        else_branch,
                    });
                }
                Terminator::Return => {
                    result.push(Structured::Return);
                }
            }
            result
        }
    }

    /// 論文の doBranch (line 22-26)
    fn do_branch(
        &self,
        source: BasicBlockId,
        target: BasicBlockId,
        context: &[ContainingSyntax],
    ) -> Vec<Structured> {
        let is_backward = self.rpo[&source] >= self.rpo[&target];

        if is_backward || self.merge_nodes.contains(&target) {
            let relative_index = self
                .index(target, context)
                .expect("Target label not found in context");
            // 後方分岐、またはマージノードへの分岐は BreakTo になる
            vec![Structured::Break(relative_index)]
        } else {
            // それ以外は、ターゲットのサブツリーをインライン展開する
            // (ターゲットはドミネーターツリーでsourceの子であるはず)
            let target_node = self
                .find_dom_tree_node(self.dom_tree, target)
                .expect("Branch target must be in the dominator tree");
            self.do_tree(target_node, context)
        }
    }

    // --- ヘルパー関数 ---
    fn find_dom_tree_node<'b>(
        &self,
        node: &'b DomTreeNode,
        id: BasicBlockId,
    ) -> Option<&'b DomTreeNode> {
        if node.id == id {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = self.find_dom_tree_node(child, id) {
                return Some(found);
            }
        }
        None
    }
}

pub fn structured_control(cfg: &CFG, entry_id: BasicBlockId) -> Vec<Structured> {
    let rpo = calculate_rpo(cfg, entry_id);
    let dom_tree = build_dom_tree(cfg, &rpo, entry_id);
    let loop_headers = find_loop_headers(cfg, &rpo);
    let merge_nodes = find_merge_nodes(cfg, &rpo);

    // 2. Translatorを初期化
    let translator = Translator {
        cfg,
        dom_tree: &dom_tree,
        rpo: &rpo,
        loop_headers: &loop_headers,
        merge_nodes: &merge_nodes,
    };

    // 3. 変換を開始
    translator.do_tree(&dom_tree, &[])
}

/// 1. 逆後順序 (RPO) 番号を計算する
fn calculate_rpo(cfg: &CFG, entry_id: BasicBlockId) -> HashMap<BasicBlockId, usize> {
    let mut visited = HashSet::new();
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

// `calculate_rpo` のための再帰的なDFSヘルパー
fn dfs_postorder(
    current_id: BasicBlockId,
    cfg: &CFG,
    visited: &mut HashSet<BasicBlockId>,
    postorder: &mut Vec<BasicBlockId>,
) {
    visited.insert(current_id);
    let node = cfg.get(&current_id).unwrap();

    // 後続ノードを探索
    let successors = match &node.terminator {
        Terminator::Jump(target) => vec![*target],
        Terminator::If {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        Terminator::Return => vec![],
    };

    for &successor in &successors {
        if !visited.contains(&successor) {
            dfs_postorder(successor, cfg, visited, postorder);
        }
    }

    // すべての子孫の訪問が終わった後 (帰りがけ) に自身を追加
    postorder.push(current_id);
}

/// 2. マージノードを特定する
fn find_merge_nodes(cfg: &CFG, rpo: &HashMap<BasicBlockId, usize>) -> HashSet<BasicBlockId> {
    let mut predecessors: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();

    // 全ノードの先行ノード(predecessor)のリストを作成
    for (&id, block) in cfg {
        let successors = match &block.terminator {
            Terminator::Jump(target) => vec![*target],
            Terminator::If {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            Terminator::Return => vec![],
        };
        for &successor in &successors {
            predecessors.entry(successor).or_default().push(id);
        }
    }

    let mut merge_nodes = HashSet::new();
    for (&id, preds) in &predecessors {
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

/// 3. ループヘッダを特定する
fn find_loop_headers(cfg: &CFG, rpo: &HashMap<BasicBlockId, usize>) -> HashSet<BasicBlockId> {
    let mut headers = HashSet::new();
    for (&source_id, block) in cfg {
        let successors = match &block.terminator {
            Terminator::Jump(target) => vec![*target],
            Terminator::If {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            Terminator::Return => vec![],
        };

        for &target_id in &successors {
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

/// 4. ドミネーターツリーを構築する
fn build_dom_tree(
    cfg: &CFG,
    rpo: &HashMap<BasicBlockId, usize>,
    entry_id: BasicBlockId,
) -> DomTreeNode {
    // --- Step A: 先行ノードのマップを作成 ---
    let mut predecessors: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    for (&id, block) in cfg {
        let successors = match &block.terminator {
            Terminator::Jump(target) => vec![*target],
            Terminator::If {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            Terminator::Return => vec![],
        };
        for &successor in &successors {
            predecessors.entry(successor).or_default().push(id);
        }
    }

    // --- Step B: データフロー解析で各ノードの支配ノード集合を計算 ---
    let all_nodes: HashSet<BasicBlockId> = cfg.keys().cloned().collect();
    let mut doms: HashMap<BasicBlockId, HashSet<BasicBlockId>> = HashMap::new();

    // 初期化
    doms.insert(entry_id, [entry_id].iter().cloned().collect());
    for &id in &all_nodes {
        if id != entry_id {
            doms.insert(id, all_nodes.clone());
        }
    }

    // RPO順で計算すると収束が速い
    let mut rpo_nodes: Vec<_> = cfg.keys().cloned().collect();
    rpo_nodes.sort_by_key(|id| rpo.get(id).unwrap());

    // 集合が変化しなくなるまで反復計算
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

    // --- Step C: 即時支配ノード (idom) を見つける ---
    let mut idoms: HashMap<BasicBlockId, BasicBlockId> = HashMap::new();
    for &id in &all_nodes {
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
    let mut children_map: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    for (child, parent) in idoms {
        children_map.entry(parent).or_default().push(child);
    }

    build_tree_recursive(entry_id, &children_map)
}

// `build_dom_tree`のための再帰的な木構築ヘルパー
fn build_tree_recursive(
    id: BasicBlockId,
    children_map: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_cfg(data: &[(BasicBlockId, Terminator)]) -> CFG {
        data.iter()
            .map(|&(id, ref term)| {
                let block = BasicBlock {
                    id,
                    terminator: term.clone(),
                };
                (id, block)
            })
            .collect()
    }

    #[test]
    fn test_linear() {
        let cfg = setup_cfg(&[(0, Terminator::Jump(1)), (1, Terminator::Return)]);

        let result = structured_control(&cfg, 0);

        let expected = vec![
            Structured::Simple(0),
            Structured::Simple(1),
            Structured::Return,
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_simple_if_else() {
        let cfg = setup_cfg(&[
            (0, Terminator::If {
                condition: 100,
                then_block: 1,
                else_block: 2,
            }),
            (1, Terminator::Jump(3)),
            (2, Terminator::Jump(3)),
            (3, Terminator::Return),
        ]);

        let result = structured_control(&cfg, 0);

        let expected = vec![
            Structured::Block {
                body: vec![Structured::Simple(0), Structured::If {
                    condition: 100,
                    then_branch: vec![Structured::Simple(1), Structured::Break(1)],
                    else_branch: vec![Structured::Simple(2), Structured::Break(1)],
                }],
            },
            Structured::Simple(3),
            Structured::Return,
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_nested_if_else() {
        let cfg = setup_cfg(&[
            (0, Terminator::If {
                condition: 100,
                then_block: 1,
                else_block: 4,
            }),
            (1, Terminator::If {
                condition: 101,
                then_block: 2,
                else_block: 3,
            }),
            (2, Terminator::Jump(5)),
            (3, Terminator::Jump(5)),
            (4, Terminator::Jump(5)),
            (5, Terminator::Return),
        ]);

        let result = structured_control(&cfg, 0);

        let expected = vec![
            Structured::Block {
                body: vec![Structured::Simple(0), Structured::If {
                    condition: 100,
                    then_branch: vec![Structured::Simple(1), Structured::If {
                        condition: 101,
                        then_branch: vec![Structured::Simple(2), Structured::Break(2)],
                        else_branch: vec![Structured::Simple(3), Structured::Break(2)],
                    }],
                    else_branch: vec![Structured::Simple(4), Structured::Break(1)],
                }],
            },
            Structured::Simple(5),
            Structured::Return,
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_simple_loop() {
        let cfg = setup_cfg(&[
            (0, Terminator::Jump(1)),
            (1, Terminator::If {
                condition: 101,
                then_block: 2,
                else_block: 3,
            }),
            (2, Terminator::Jump(1)),
            (3, Terminator::Return),
        ]);

        let result = structured_control(&cfg, 0);

        let expected = vec![Structured::Simple(0), Structured::Loop {
            body: vec![Structured::Simple(1), Structured::If {
                condition: 101,
                then_branch: vec![
                    Structured::Simple(2),
                    Structured::Break(1), // ループ継続
                ],
                else_branch: vec![
                    Structured::Simple(3),
                    Structured::Return, // ループ脱出
                ],
            }],
        }];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_simple_infinite_loop() {
        let cfg = setup_cfg(&[(0, Terminator::Jump(1)), (1, Terminator::Jump(0))]);

        let result = structured_control(&cfg, 0);

        let expected = vec![Structured::Loop {
            body: vec![
                Structured::Simple(0),
                Structured::Simple(1),
                Structured::Break(0),
            ],
        }];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_self_loop() {
        let cfg = setup_cfg(&[
            (0, Terminator::If {
                condition: 100,
                then_block: 1,
                else_block: 2,
            }),
            (1, Terminator::If {
                condition: 101,
                then_block: 1,
                else_block: 2,
            }),
            (2, Terminator::Return),
        ]);

        let result = structured_control(&cfg, 0);

        let expected = vec![
            Structured::Block {
                body: vec![Structured::Simple(0), Structured::If {
                    condition: 100,
                    then_branch: vec![Structured::Loop {
                        body: vec![Structured::Simple(1), Structured::If {
                            condition: 101,
                            then_branch: vec![Structured::Break(1)],
                            else_branch: vec![Structured::Break(3)],
                        }],
                    }],
                    else_branch: vec![Structured::Break(1)],
                }],
            },
            Structured::Simple(2),
            Structured::Return,
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_self_infinite_loop() {
        let cfg = setup_cfg(&[(0, Terminator::Jump(0))]);

        let result = structured_control(&cfg, 0);

        let expected = vec![Structured::Loop {
            body: vec![Structured::Simple(0), Structured::Break(0)],
        }];

        assert_eq!(result, expected);
    }

    #[test]
    #[should_panic(expected = "Target label not found in context")]
    fn test_irreducible_graph_fails() {
        // irreducible graphでは動かない
        // ノード分割が必要だが、ここでは実装しない
        let cfg = setup_cfg(&[
            (0, Terminator::If {
                condition: 100,
                then_block: 1,
                else_block: 2,
            }),
            (1, Terminator::Jump(2)),
            (2, Terminator::Jump(1)),
        ]);

        structured_control(&cfg, 0);
    }

    // 論文の複雑な合流パターン
    #[test]
    fn test_unusual_merge_pattern() {
        let cfg = setup_cfg(&[
            (0, Terminator::If {
                condition: 100,
                then_block: 1,
                else_block: 3,
            }), // A
            (1, Terminator::If {
                condition: 101,
                then_block: 2,
                else_block: 4,
            }), // B
            (2, Terminator::Jump(5)), // C
            (3, Terminator::If {
                condition: 102,
                then_block: 4,
                else_block: 5,
            }), // D
            (4, Terminator::Jump(5)), // E (Merge Node)
            (5, Terminator::Return),  // F (Merge Node)
        ]);

        let result = structured_control(&cfg, 0);

        let expected = vec![
            Structured::Block {
                body: vec![
                    Structured::Block {
                        body: vec![
                            Structured::Simple(0), // A
                            Structured::If {
                                condition: 100,
                                then_branch: vec![Structured::Simple(1), Structured::If {
                                    condition: 101,
                                    then_branch: vec![Structured::Simple(2), Structured::Break(3)], // C -> F
                                    else_branch: vec![Structured::Break(2)], // B -> E
                                }],
                                else_branch: vec![Structured::Simple(3), Structured::If {
                                    condition: 102,
                                    then_branch: vec![Structured::Break(2)], // D -> E
                                    else_branch: vec![Structured::Break(3)], // D -> F
                                }],
                            },
                        ],
                    },
                    Structured::Simple(4), // E
                    Structured::Break(0),  // E -> F
                ],
            },
            Structured::Simple(5), // F
            Structured::Return,
        ];

        assert_eq!(result, expected);
    }
}
