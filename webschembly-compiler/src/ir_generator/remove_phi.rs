use crate::ir::{BasicBlockId, Expr, ExprAssign, Func, Local, LocalId};
use rustc_hash::{FxHashMap, FxHashSet};

// 前提条件: クリティカルエッジが存在しない
pub fn remove_phi(func: &mut Func) {
    // 各ベーシックブロックのPHI命令を処理
    let bb_ids: Vec<_> = func.bbs.keys().collect();

    for bb_id in bb_ids {
        remove_phi_in_bb(func, bb_id);
    }
}

#[derive(Debug, Clone, Copy)]
struct Copy {
    pub dest: LocalId,
    pub src: LocalId,
}

fn remove_phi_in_bb(func: &mut Func, bb_id: BasicBlockId) {
    // 先行ブロックごとのコピーリスト(並列コピーセマンティクス)
    let mut pending_copies: FxHashMap<BasicBlockId, Vec<Copy>> = FxHashMap::default();

    // 先行ブロックごとの並列コピーリストを収集
    for expr_assign in &func.bbs[bb_id].exprs {
        if let Expr::Phi(incomings) = &expr_assign.expr
            && let Some(result) = expr_assign.local
        {
            for incoming in incomings {
                pending_copies
                    .entry(incoming.bb)
                    .or_insert_with(Vec::new)
                    .push(Copy {
                        dest: result,
                        src: incoming.local,
                    });
            }
        }
    }

    // 並列コピーを逐次コピーに変換して、先行ブロックの末尾に挿入
    for (block_id, parallel_copies) in pending_copies {
        let sequential_copies = sequentialize_parallel_copies(func, parallel_copies);
        let bb = &mut func.bbs[block_id];

        for copy in sequential_copies {
            bb.exprs.push(ExprAssign {
                local: Some(copy.dest),
                expr: Expr::Move(copy.src),
            });
        }
    }

    // 対象ブロックのPHI命令を削除
    for expr_assign in &mut func.bbs[bb_id].exprs {
        if let Expr::Phi(_) = &expr_assign.expr {
            expr_assign.expr = Expr::Nop;
            expr_assign.local = None;
        }
    }
}

// 並列コピーを逐次コピーに変換
fn sequentialize_parallel_copies(func: &mut Func, parallel_copies: Vec<Copy>) -> Vec<Copy> {
    let mut todo = parallel_copies;
    let mut result = Vec::new();
    while !todo.is_empty() {
        let sources_in_todo = todo.iter().map(|c| c.src).collect::<FxHashSet<_>>();
        let mut ready_copies = Vec::new();
        let mut not_ready_copies = Vec::new();
        for copy in todo.drain(..) {
            if sources_in_todo.contains(&copy.dest) {
                not_ready_copies.push(copy);
            } else {
                ready_copies.push(copy);
            }
        }
        if !ready_copies.is_empty() {
            result.extend(&ready_copies);
            todo = not_ready_copies;
        } else {
            let cycle_copy = not_ready_copies.pop().unwrap();
            debug_assert_eq!(
                func.locals[cycle_copy.src].typ,
                func.locals[cycle_copy.dest].typ
            );
            let typ = func.locals[cycle_copy.src].typ;
            let temp = func.locals.push_with(|id| Local { id, typ });
            result.push(Copy {
                dest: temp,
                src: cycle_copy.src,
            });
            not_ready_copies.push(Copy {
                dest: cycle_copy.dest,
                src: temp,
            });
            todo = not_ready_copies;
        }
    }
    result
}
