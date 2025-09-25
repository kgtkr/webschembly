use crate::ir::{BasicBlock, BasicBlockId, BasicBlockNext, ExprCall, ExprCallRef, Func, LocalId};
use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::TiSlice;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredBlock {
    Simple {
        id: BasicBlockId,
    },
    If {
        condition: LocalId,
        then_blocks: Vec<StructuredBlock>,
        else_blocks: Vec<StructuredBlock>,
    },
    Loop {
        id: u32,
        body: Vec<StructuredBlock>,
    },
    Break {
        loop_id: u32,
    },
    Continue {
        loop_id: u32,
    },
    Return {
        value: LocalId,
    },
    TailCall {
        call: ExprCall,
    },
    TailCallRef {
        call: ExprCallRef,
    },
}

#[derive(Debug)]
pub struct Relooper {
    processed: FxHashSet<BasicBlockId>,
    predecessors: FxHashMap<BasicBlockId, FxHashSet<BasicBlockId>>,
    reachable: FxHashSet<BasicBlockId>,
    dfs_order: FxHashMap<BasicBlockId, usize>,
    dfs_parent: FxHashMap<BasicBlockId, Option<BasicBlockId>>,
    next_loop_id: u32,
}

impl Relooper {
    pub fn new() -> Self {
        Self {
            processed: FxHashSet::default(),
            predecessors: FxHashMap::default(),
            reachable: FxHashSet::default(),
            dfs_order: FxHashMap::default(),
            dfs_parent: FxHashMap::default(),
            next_loop_id: 0,
        }
    }

    fn generate_loop_id(&mut self) -> u32 {
        let id = self.next_loop_id;
        self.next_loop_id += 1;
        id
    }

    pub fn reloop(&mut self, func: &Func) -> Vec<StructuredBlock> {
        self.analyze_control_flow(func);
        self.compute_dfs_order(func.bb_entry, &func.bbs);
        self.process_blocks(func.bb_entry, &func.bbs)
    }

    fn analyze_control_flow(&mut self, func: &Func) {
        for bb in func.bbs.iter() {
            for successor in bb.next.successors() {
                self.predecessors
                    .entry(successor)
                    .or_default()
                    .insert(bb.id);
            }
        }

        self.compute_reachability(func.bb_entry, &func.bbs);
    }

    fn compute_dfs_order(&mut self, entry: BasicBlockId, bbs: &TiSlice<BasicBlockId, BasicBlock>) {
        self.dfs_order.clear();
        self.dfs_parent.clear();

        let mut visited = FxHashSet::default();
        let mut order_counter = 0;

        self.dfs_visit(entry, None, bbs, &mut visited, &mut order_counter);
    }

    fn dfs_visit(
        &mut self,
        current: BasicBlockId,
        parent: Option<BasicBlockId>,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
        visited: &mut FxHashSet<BasicBlockId>,
        order_counter: &mut usize,
    ) {
        if visited.contains(&current) {
            return;
        }

        visited.insert(current);
        self.dfs_order.insert(current, *order_counter);
        self.dfs_parent.insert(current, parent);
        *order_counter += 1;

        let bb = &bbs[current];
        for successor in bb.next.successors() {
            if !visited.contains(&successor) {
                self.dfs_visit(successor, Some(current), bbs, visited, order_counter);
            }
        }
    }

    fn compute_reachability(
        &mut self,
        entry: BasicBlockId,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
    ) {
        let mut stack = vec![entry];
        self.reachable.insert(entry);

        while let Some(current) = stack.pop() {
            let bb = &bbs[current];
            for successor in bb.next.successors() {
                if !self.reachable.contains(&successor) {
                    self.reachable.insert(successor);
                    stack.push(successor);
                }
            }
        }
    }

    fn process_blocks(
        &mut self,
        entry: BasicBlockId,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
    ) -> Vec<StructuredBlock> {
        let mut result = Vec::new();
        self.process_block_recursive(entry, bbs, &mut result);
        result
    }

    fn process_block_recursive(
        &mut self,
        block_id: BasicBlockId,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
        result: &mut Vec<StructuredBlock>,
    ) {
        if self.processed.contains(&block_id) {
            return;
        }

        if !self.reachable.contains(&block_id) {
            return;
        }

        self.processed.insert(block_id);
        let bb = &bbs[block_id];

        result.push(StructuredBlock::Simple { id: block_id });

        match &bb.next {
            BasicBlockNext::If(condition, then_id, else_id) => {
                self.handle_if_block(block_id, *condition, *then_id, *else_id, bbs, result);
            }
            BasicBlockNext::Jump(target) => {
                self.process_block_recursive(*target, bbs, result);
            }
            BasicBlockNext::Return(value) => {
                result.push(StructuredBlock::Return { value: *value });
            }
            BasicBlockNext::TailCall(call) => {
                result.push(StructuredBlock::TailCall { call: call.clone() });
            }
            BasicBlockNext::TailCallRef(call_ref) => {
                result.push(StructuredBlock::TailCallRef {
                    call: call_ref.clone(),
                });
            }
        }
    }

    fn handle_if_block(
        &mut self,
        current_block: BasicBlockId,
        condition: LocalId,
        then_id: BasicBlockId,
        else_id: BasicBlockId,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
        result: &mut Vec<StructuredBlock>,
    ) {
        let mut then_blocks = Vec::new();
        let mut else_blocks = Vec::new();

        // バックエッジかどうかをまず確認
        if self.is_back_edge(current_block, then_id, else_id, bbs) {
            // ループの場合：then側はループ継続、else側はループ脱出
            let loop_id = self.generate_loop_id();

            // ループのbodyを構築
            let mut loop_body = Vec::new();
            if then_id == current_block {
                // 自己ループの場合、ループ本体は空（条件チェックのみ）
                // 実際のループ継続条件はIf文で表現される
            } else {
                // 他のブロックへのバックエッジの場合、そのブロックから現在のブロックまでがループ本体
                if !self.processed.contains(&then_id) {
                    self.process_block_recursive(then_id, bbs, &mut loop_body);
                }
            }

            // then側にループを追加
            then_blocks.push(StructuredBlock::Loop {
                id: loop_id,
                body: loop_body,
            });

            // else側（ループ脱出）を処理
            if !self.processed.contains(&else_id) {
                self.process_block_recursive(else_id, bbs, &mut else_blocks);
            }

            result.push(StructuredBlock::If {
                condition,
                then_blocks,
                else_blocks,
            });
        } else {
            // 通常のif-else処理
            let convergence_point = self.find_convergence_point(then_id, else_id, bbs);

            if !self.processed.contains(&then_id) {
                self.process_block_until_convergence(
                    then_id,
                    convergence_point,
                    bbs,
                    &mut then_blocks,
                );
            }

            if !self.processed.contains(&else_id) {
                self.process_block_until_convergence(
                    else_id,
                    convergence_point,
                    bbs,
                    &mut else_blocks,
                );
            }

            result.push(StructuredBlock::If {
                condition,
                then_blocks,
                else_blocks,
            });

            // 合流ポイントがある場合、それを処理
            if let Some(conv_point) = convergence_point {
                if !self.processed.contains(&conv_point) {
                    self.process_block_recursive(conv_point, bbs, result);
                }
            }
        }
    }

    fn is_back_edge(
        &self,
        current_block: BasicBlockId,
        then_id: BasicBlockId,
        else_id: BasicBlockId,
        _bbs: &TiSlice<BasicBlockId, BasicBlock>,
    ) -> bool {
        // バックエッジ検出:
        // 1. 自己ループ（then_id == current_block）
        // 2. then側がエントリーノード（block0）に戻る場合
        // 3. 既に処理済みの祖先ノードへの戻り

        // 自己ループの検出
        if then_id == current_block {
            return true;
        }

        if let Some(&then_order) = self.dfs_order.get(&then_id) {
            // then側がエントリーノード（block0）への戻りはループ
            if then_order == 0 {
                return true;
            }
        }

        // より包括的なバックエッジ検出:
        // 現在処理中のブロックのDFS順序より小さい順序のノードへの参照はバックエッジ
        if let (Some(&then_order), Some(&_else_order)) =
            (self.dfs_order.get(&then_id), self.dfs_order.get(&else_id))
        {
            // 両方のブランチが同じノード（収束）で、そのノードが祖先の場合
            if then_id == else_id {
                // 現在のDFS順序の最大値を取得
                let current_orders: Vec<usize> = self.dfs_order.values().copied().collect();
                let max_current = current_orders.iter().max().unwrap_or(&0);
                return then_order < *max_current;
            }
        }

        false
    }

    fn create_loop_structure(
        &mut self,
        _header: BasicBlockId,
        _func: &Func,
        result: &mut Vec<StructuredBlock>,
    ) {
        let loop_body = Vec::new();
        let loop_id = self.generate_loop_id();

        result.push(StructuredBlock::Loop {
            id: loop_id,
            body: loop_body,
        });
    }

    fn find_convergence_point(
        &self,
        then_id: BasicBlockId,
        else_id: BasicBlockId,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
    ) -> Option<BasicBlockId> {
        // 簡単な実装: 両方のブランチが単純なジャンプで同じターゲットに向かう場合
        let then_bb = &bbs[then_id];
        let else_bb = &bbs[else_id];

        match (&then_bb.next, &else_bb.next) {
            (BasicBlockNext::Jump(then_target), BasicBlockNext::Jump(else_target)) => {
                if then_target == else_target {
                    Some(*then_target)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn process_block_until_convergence(
        &mut self,
        block_id: BasicBlockId,
        convergence_point: Option<BasicBlockId>,
        bbs: &TiSlice<BasicBlockId, BasicBlock>,
        result: &mut Vec<StructuredBlock>,
    ) {
        if self.processed.contains(&block_id) {
            return;
        }

        if !self.reachable.contains(&block_id) {
            return;
        }

        // 合流ポイントに到達した場合は処理を停止
        if let Some(conv_point) = convergence_point {
            if block_id == conv_point {
                return;
            }
        }

        self.processed.insert(block_id);
        let bb = &bbs[block_id];

        result.push(StructuredBlock::Simple { id: block_id });

        match &bb.next {
            BasicBlockNext::Jump(target) => {
                // 合流ポイントへのジャンプの場合は停止
                if let Some(conv_point) = convergence_point {
                    if *target == conv_point {
                        return;
                    }
                }
                self.process_block_until_convergence(*target, convergence_point, bbs, result);
            }
            BasicBlockNext::If(condition, then_id, else_id) => {
                self.handle_if_block(block_id, *condition, *then_id, *else_id, bbs, result);
            }
            BasicBlockNext::Return(value) => {
                result.push(StructuredBlock::Return { value: *value });
            }
            BasicBlockNext::TailCall(call) => {
                result.push(StructuredBlock::TailCall { call: call.clone() });
            }
            BasicBlockNext::TailCallRef(call_ref) => {
                result.push(StructuredBlock::TailCallRef {
                    call: call_ref.clone(),
                });
            }
        }
    }
}

pub fn reloop_function(func: &Func) -> Vec<StructuredBlock> {
    let mut relooper = Relooper::new();
    relooper.reloop(func)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{FuncId, LocalType, Type, ValType};
    use StructuredBlock::*;
    use typed_index_collections::TiVec;

    fn create_test_func(
        locals: Vec<LocalType>,
        args: Vec<LocalId>,
        ret_type: LocalType,
        blocks: Vec<BasicBlockNext>,
    ) -> Func {
        let func_id = FuncId::from(0);
        let mut bbs = TiVec::new();
        let mut bb_ids = Vec::new();

        for (i, next) in blocks.iter().enumerate() {
            let bb_id = BasicBlockId::from(i);
            bb_ids.push(bb_id);

            let bb = BasicBlock {
                id: bb_id,
                exprs: Vec::new(),
                next: next.clone(),
            };
            bbs.push(bb);
        }

        Func {
            id: func_id,
            locals: TiVec::from_iter(locals),
            args,
            ret_type,
            bb_entry: BasicBlockId::from(0),
            bbs,
        }
    }

    #[test]
    fn test_simple_linear_flow() {
        let local0 = LocalId::from(0);
        let local1 = LocalId::from(1);

        let blocks = vec![
            BasicBlockNext::Jump(BasicBlockId::from(1)),
            BasicBlockNext::Return(local1),
        ];

        let func = create_test_func(
            vec![
                LocalType::Type(Type::Val(ValType::Int)),
                LocalType::Type(Type::Val(ValType::Int)),
            ],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            Simple {
                id: BasicBlockId::from(1)
            },
            Return {
                value: LocalId::from(1)
            }
        ]);
    }

    #[test]
    fn test_if_else_flow() {
        let local0 = LocalId::from(0);
        let local1 = LocalId::from(1);
        let local2 = LocalId::from(2);

        let blocks = vec![
            BasicBlockNext::If(local0, BasicBlockId::from(1), BasicBlockId::from(2)),
            BasicBlockNext::Return(local1),
            BasicBlockNext::Return(local2),
        ];
        let func = create_test_func(
            vec![
                LocalType::Type(Type::Val(ValType::Bool)),
                LocalType::Type(Type::Val(ValType::Int)),
                LocalType::Type(Type::Val(ValType::Int)),
            ],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);
        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            If {
                condition: LocalId::from(0),
                then_blocks: vec![
                    Simple {
                        id: BasicBlockId::from(1)
                    },
                    Return {
                        value: LocalId::from(1)
                    }
                ],
                else_blocks: vec![
                    Simple {
                        id: BasicBlockId::from(2)
                    },
                    Return {
                        value: LocalId::from(2)
                    }
                ]
            }
        ]);
    }

    #[test]
    fn test_tail_call() {
        let local0 = LocalId::from(0);
        let func_id = FuncId::from(1);

        let call = ExprCall {
            func_id,
            args: vec![local0],
        };

        let blocks = vec![BasicBlockNext::TailCall(call.clone())];

        let func = create_test_func(
            vec![LocalType::Type(Type::Val(ValType::Int))],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            TailCall { call }
        ]);
    }

    #[test]
    fn test_reloop_function_helper() {
        let local0 = LocalId::from(0);

        let blocks = vec![BasicBlockNext::Return(local0)];

        let func = create_test_func(
            vec![LocalType::Type(Type::Val(ValType::Int))],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let result = reloop_function(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            Return {
                value: LocalId::from(0)
            }
        ]);
    }

    #[test]
    fn test_complex_control_flow() {
        let local0 = LocalId::from(0);
        let local1 = LocalId::from(1);

        let blocks = vec![
            BasicBlockNext::If(local0, BasicBlockId::from(1), BasicBlockId::from(2)),
            BasicBlockNext::Jump(BasicBlockId::from(3)),
            BasicBlockNext::Jump(BasicBlockId::from(3)),
            BasicBlockNext::Return(local1),
        ];

        let func = create_test_func(
            vec![
                LocalType::Type(Type::Val(ValType::Bool)),
                LocalType::Type(Type::Val(ValType::Int)),
            ],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            If {
                condition: LocalId::from(0),
                then_blocks: vec![Simple {
                    id: BasicBlockId::from(1)
                }],
                else_blocks: vec![Simple {
                    id: BasicBlockId::from(2)
                }]
            },
            Simple {
                id: BasicBlockId::from(3)
            },
            Return {
                value: LocalId::from(1)
            }
        ]);
    }

    #[test]
    fn test_empty_blocks() {
        let local0 = LocalId::from(0);

        let blocks = vec![BasicBlockNext::Return(local0)];

        let func = create_test_func(
            vec![LocalType::Type(Type::Val(ValType::Int))],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            Return {
                value: LocalId::from(0)
            }
        ]);
    }

    #[test]
    fn test_simple_loop() {
        let local0 = LocalId::from(0);
        let local1 = LocalId::from(1);

        let blocks = vec![
            BasicBlockNext::If(local0, BasicBlockId::from(1), BasicBlockId::from(2)),
            BasicBlockNext::If(local0, BasicBlockId::from(1), BasicBlockId::from(2)),
            BasicBlockNext::Return(local1),
        ];

        let func = create_test_func(
            vec![
                LocalType::Type(Type::Val(ValType::Bool)),
                LocalType::Type(Type::Val(ValType::Int)),
            ],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        // TODO: おかしい
        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            If {
                condition: LocalId::from(0),
                then_blocks: vec![
                    Simple {
                        id: BasicBlockId::from(1)
                    },
                    If {
                        condition: LocalId::from(0),
                        then_blocks: vec![Loop {
                            id: 0,
                            body: vec![]
                        }],
                        else_blocks: vec![
                            Simple {
                                id: BasicBlockId::from(2)
                            },
                            Return {
                                value: LocalId::from(1)
                            }
                        ]
                    }
                ],
                else_blocks: vec![]
            }
        ]);
    }

    #[test]
    fn test_convergent_branches() {
        let local0 = LocalId::from(0);
        let local1 = LocalId::from(1);

        let blocks = vec![
            BasicBlockNext::If(local0, BasicBlockId::from(1), BasicBlockId::from(2)),
            BasicBlockNext::Return(local1),
            BasicBlockNext::Jump(BasicBlockId::from(1)),
        ];

        let func = create_test_func(
            vec![
                LocalType::Type(Type::Val(ValType::Bool)),
                LocalType::Type(Type::Val(ValType::Int)),
            ],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            If {
                condition: LocalId::from(0),
                then_blocks: vec![
                    Simple {
                        id: BasicBlockId::from(1)
                    },
                    Return {
                        value: LocalId::from(1)
                    }
                ],
                // TODO: おかしい
                else_blocks: vec![Simple {
                    id: BasicBlockId::from(2)
                }]
            }
        ]);
    }

    #[test]
    fn test_actual_loop_detection() {
        let local0 = LocalId::from(0);
        let local1 = LocalId::from(1);

        let blocks = vec![
            BasicBlockNext::If(local0, BasicBlockId::from(0), BasicBlockId::from(1)),
            BasicBlockNext::Return(local1),
        ];

        let func = create_test_func(
            vec![
                LocalType::Type(Type::Val(ValType::Bool)),
                LocalType::Type(Type::Val(ValType::Int)),
            ],
            vec![local0],
            LocalType::Type(Type::Val(ValType::Int)),
            blocks,
        );

        let mut relooper = Relooper::new();
        let result = relooper.reloop(&func);

        assert_eq!(result, vec![
            Simple {
                id: BasicBlockId::from(0)
            },
            Loop {
                id: 0,
                body: vec![]
            },
            Simple {
                id: BasicBlockId::from(1)
            },
            Return {
                value: LocalId::from(1)
            }
        ]);
    }
}
