// https://dl.acm.org/doi/abs/10.1145/3547621
// by gemini

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{BasicBlock, BasicBlockId, BasicBlockNext, BasicBlockTerminator, Func, LocalId};
use crate::ir_processor::cfg_analyzer::{
    DomTreeNode, build_dom_tree, calc_doms, calc_predecessors, calculate_rpo, find_loop_headers,
    find_merge_nodes,
};
use vec_map::VecMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Structured {
    Simple(BasicBlockId),
    If {
        cond: LocalId,
        then: Vec<Structured>,
        else_: Vec<Structured>,
    },
    Block {
        body: Vec<Structured>,
    },
    Loop {
        body: Vec<Structured>,
    },
    Break(usize),
    Terminator(BasicBlockTerminator),
}

#[derive(Debug, Clone, Copy)]
enum ContainingSyntax {
    IfThenElse,
    LoopHeadedBy(BasicBlockId),
    BlockFollowedBy(BasicBlockId),
}

struct Translator<'a> {
    cfg: &'a VecMap<BasicBlockId, BasicBlock>,
    dom_tree: &'a DomTreeNode,
    rpo: &'a FxHashMap<BasicBlockId, usize>,
    loop_headers: &'a FxHashSet<BasicBlockId>,
    merge_nodes: &'a FxHashSet<BasicBlockId>,
}

impl Translator<'_> {
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
            let block = self.cfg.get(x).unwrap();

            let mut result = Vec::new();
            result.push(Structured::Simple(block.id));

            match &block.next {
                BasicBlockNext::Jump(target) => {
                    result.extend(self.do_branch(x, *target, context));
                }
                BasicBlockNext::If(condition, then_block, else_block) => {
                    let mut new_context = context.to_vec();
                    new_context.insert(0, ContainingSyntax::IfThenElse);

                    let then_branch = self.do_branch(x, *then_block, &new_context);
                    let else_branch = self.do_branch(x, *else_block, &new_context);

                    result.push(Structured::If {
                        cond: *condition,
                        then: then_branch,
                        else_: else_branch,
                    });
                }
                BasicBlockNext::Terminator(terminator) => {
                    result.push(Structured::Terminator(terminator.clone()));
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
            let target_node = Self::find_dom_tree_node(self.dom_tree, target)
                .expect("Branch target must be in the dominator tree");
            self.do_tree(target_node, context)
        }
    }

    // --- ヘルパー関数 ---
    fn find_dom_tree_node(node: &DomTreeNode, id: BasicBlockId) -> Option<&DomTreeNode> {
        if node.id == id {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = Self::find_dom_tree_node(child, id) {
                return Some(found);
            }
        }
        None
    }
}

pub fn reloop(func: &Func) -> Vec<Structured> {
    reloop_cfg(&func.bbs, func.bb_entry)
}

fn reloop_cfg(cfg: &VecMap<BasicBlockId, BasicBlock>, entry_id: BasicBlockId) -> Vec<Structured> {
    let predecessors = calc_predecessors(cfg);
    let rpo = calculate_rpo(cfg, entry_id);
    let doms = calc_doms(cfg, &rpo, entry_id, &predecessors);
    let dom_tree = build_dom_tree(cfg, &rpo, entry_id, &doms);
    let loop_headers = find_loop_headers(cfg, &rpo);
    let merge_nodes = find_merge_nodes(&rpo, &predecessors);

    let translator = Translator {
        cfg,
        dom_tree: &dom_tree,
        rpo: &rpo,
        loop_headers: &loop_headers,
        merge_nodes: &merge_nodes,
    };

    translator.do_tree(&dom_tree, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_cfg(data: &[(usize, BasicBlockNext)]) -> VecMap<BasicBlockId, BasicBlock> {
        data.iter()
            .map(|&(id, ref next)| {
                let block_id = BasicBlockId::from(id);
                let block = BasicBlock {
                    id: block_id,
                    exprs: vec![],
                    next: next.clone(),
                };
                (block_id, block)
            })
            .collect()
    }

    #[test]
    fn test_linear() {
        let cfg = setup_cfg(&[
            (0, BasicBlockNext::Jump(BasicBlockId::from(1))),
            (
                1,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ),
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![
            Structured::Simple(BasicBlockId::from(0)),
            Structured::Simple(BasicBlockId::from(1)),
            Structured::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_simple_if_else() {
        let cfg = setup_cfg(&[
            (
                0,
                BasicBlockNext::If(
                    LocalId::from(100),
                    BasicBlockId::from(1),
                    BasicBlockId::from(2),
                ),
            ),
            (1, BasicBlockNext::Jump(BasicBlockId::from(3))),
            (2, BasicBlockNext::Jump(BasicBlockId::from(3))),
            (
                3,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ),
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![
            Structured::Block {
                body: vec![
                    Structured::Simple(BasicBlockId::from(0)),
                    Structured::If {
                        cond: LocalId::from(100),
                        then: vec![
                            Structured::Simple(BasicBlockId::from(1)),
                            Structured::Break(1),
                        ],
                        else_: vec![
                            Structured::Simple(BasicBlockId::from(2)),
                            Structured::Break(1),
                        ],
                    },
                ],
            },
            Structured::Simple(BasicBlockId::from(3)),
            Structured::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_nested_if_else() {
        let cfg = setup_cfg(&[
            (
                0,
                BasicBlockNext::If(
                    LocalId::from(100),
                    BasicBlockId::from(1),
                    BasicBlockId::from(4),
                ),
            ),
            (
                1,
                BasicBlockNext::If(
                    LocalId::from(101),
                    BasicBlockId::from(2),
                    BasicBlockId::from(3),
                ),
            ),
            (2, BasicBlockNext::Jump(BasicBlockId::from(5))),
            (3, BasicBlockNext::Jump(BasicBlockId::from(5))),
            (4, BasicBlockNext::Jump(BasicBlockId::from(5))),
            (
                5,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ),
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![
            Structured::Block {
                body: vec![
                    Structured::Simple(BasicBlockId::from(0)),
                    Structured::If {
                        cond: LocalId::from(100),
                        then: vec![
                            Structured::Simple(BasicBlockId::from(1)),
                            Structured::If {
                                cond: LocalId::from(101),
                                then: vec![
                                    Structured::Simple(BasicBlockId::from(2)),
                                    Structured::Break(2),
                                ],
                                else_: vec![
                                    Structured::Simple(BasicBlockId::from(3)),
                                    Structured::Break(2),
                                ],
                            },
                        ],
                        else_: vec![
                            Structured::Simple(BasicBlockId::from(4)),
                            Structured::Break(1),
                        ],
                    },
                ],
            },
            Structured::Simple(BasicBlockId::from(5)),
            Structured::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_simple_loop() {
        let cfg = setup_cfg(&[
            (0, BasicBlockNext::Jump(BasicBlockId::from(1))),
            (
                1,
                BasicBlockNext::If(
                    LocalId::from(101),
                    BasicBlockId::from(2),
                    BasicBlockId::from(3),
                ),
            ),
            (2, BasicBlockNext::Jump(BasicBlockId::from(1))),
            (
                3,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ),
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![
            Structured::Simple(BasicBlockId::from(0)),
            Structured::Loop {
                body: vec![
                    Structured::Simple(BasicBlockId::from(1)),
                    Structured::If {
                        cond: LocalId::from(101),
                        then: vec![
                            Structured::Simple(BasicBlockId::from(2)),
                            Structured::Break(1), // ループ継続
                        ],
                        else_: vec![
                            Structured::Simple(BasicBlockId::from(3)),
                            Structured::Terminator(BasicBlockTerminator::Return(LocalId::from(
                                500,
                            ))), // ループ脱出
                        ],
                    },
                ],
            },
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_simple_infinite_loop() {
        let cfg = setup_cfg(&[
            (0, BasicBlockNext::Jump(BasicBlockId::from(1))),
            (1, BasicBlockNext::Jump(BasicBlockId::from(0))),
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![Structured::Loop {
            body: vec![
                Structured::Simple(BasicBlockId::from(0)),
                Structured::Simple(BasicBlockId::from(1)),
                Structured::Break(0),
            ],
        }];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_self_loop() {
        let cfg = setup_cfg(&[
            (
                0,
                BasicBlockNext::If(
                    LocalId::from(100),
                    BasicBlockId::from(1),
                    BasicBlockId::from(2),
                ),
            ),
            (
                1,
                BasicBlockNext::If(
                    LocalId::from(101),
                    BasicBlockId::from(1),
                    BasicBlockId::from(2),
                ),
            ),
            (
                2,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ),
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![
            Structured::Block {
                body: vec![
                    Structured::Simple(BasicBlockId::from(0)),
                    Structured::If {
                        cond: LocalId::from(100),
                        then: vec![Structured::Loop {
                            body: vec![
                                Structured::Simple(BasicBlockId::from(1)),
                                Structured::If {
                                    cond: LocalId::from(101),
                                    then: vec![Structured::Break(1)],
                                    else_: vec![Structured::Break(3)],
                                },
                            ],
                        }],
                        else_: vec![Structured::Break(1)],
                    },
                ],
            },
            Structured::Simple(BasicBlockId::from(2)),
            Structured::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_self_infinite_loop() {
        let cfg = setup_cfg(&[(0, BasicBlockNext::Jump(BasicBlockId::from(0)))]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![Structured::Loop {
            body: vec![
                Structured::Simple(BasicBlockId::from(0)),
                Structured::Break(0),
            ],
        }];

        assert_eq!(result, expected);
    }

    #[test]
    #[should_panic(expected = "Target label not found in context")]
    fn test_irreducible_graph_fails() {
        // irreducible graphでは動かない
        // ノード分割が必要だが、ここでは実装しない
        let cfg = setup_cfg(&[
            (
                0,
                BasicBlockNext::If(
                    LocalId::from(100),
                    BasicBlockId::from(1),
                    BasicBlockId::from(2),
                ),
            ),
            (1, BasicBlockNext::Jump(BasicBlockId::from(2))),
            (2, BasicBlockNext::Jump(BasicBlockId::from(1))),
        ]);

        reloop_cfg(&cfg, BasicBlockId::from(0));
    }

    // 論文の複雑な合流パターン
    #[test]
    fn test_unusual_merge_pattern() {
        let cfg = setup_cfg(&[
            (
                0,
                BasicBlockNext::If(
                    LocalId::from(100),
                    BasicBlockId::from(1),
                    BasicBlockId::from(3),
                ),
            ), // A
            (
                1,
                BasicBlockNext::If(
                    LocalId::from(101),
                    BasicBlockId::from(2),
                    BasicBlockId::from(4),
                ),
            ), // B
            (2, BasicBlockNext::Jump(BasicBlockId::from(5))), // C
            (
                3,
                BasicBlockNext::If(
                    LocalId::from(102),
                    BasicBlockId::from(4),
                    BasicBlockId::from(5),
                ),
            ), // D
            (4, BasicBlockNext::Jump(BasicBlockId::from(5))), // E (Merge Node)
            (
                5,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ), // F (Merge Node)
        ]);

        let result = reloop_cfg(&cfg, BasicBlockId::from(0));

        let expected = vec![
            Structured::Block {
                body: vec![
                    Structured::Block {
                        body: vec![
                            Structured::Simple(BasicBlockId::from(0)), // A
                            Structured::If {
                                cond: LocalId::from(100),
                                then: vec![
                                    Structured::Simple(BasicBlockId::from(1)),
                                    Structured::If {
                                        cond: LocalId::from(101),
                                        then: vec![
                                            Structured::Simple(BasicBlockId::from(2)),
                                            Structured::Break(3),
                                        ], // C -> F
                                        else_: vec![Structured::Break(2)], // B -> E
                                    },
                                ],
                                else_: vec![
                                    Structured::Simple(BasicBlockId::from(3)),
                                    Structured::If {
                                        cond: LocalId::from(102),
                                        then: vec![Structured::Break(2)], // D -> E
                                        else_: vec![Structured::Break(3)], // D -> F
                                    },
                                ],
                            },
                        ],
                    },
                    Structured::Simple(BasicBlockId::from(4)), // E
                    Structured::Break(0),                      // E -> F
                ],
            },
            Structured::Simple(BasicBlockId::from(5)), // F
            Structured::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    #[should_panic(expected = "RPO must contain all nodes")]
    fn test_unreadable_bb() {
        let cfg = setup_cfg(&[
            (0, BasicBlockNext::Jump(BasicBlockId::from(2))),
            (1, BasicBlockNext::Jump(BasicBlockId::from(2))),
            (
                2,
                BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(500))),
            ),
        ]);

        reloop_cfg(&cfg, BasicBlockId::from(0));
    }
}
