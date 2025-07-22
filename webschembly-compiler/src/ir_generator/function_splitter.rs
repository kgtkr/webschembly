use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::*;
use typed_index_collections::TiVec;
#[derive(Debug, Clone, Default)]
struct AnalyzeResult {
    defined_locals: FxHashSet<LocalId>,
    used_locals: FxHashSet<LocalId>,
}

fn analyze_locals(func: &mut Func) -> TiVec<BasicBlockId, AnalyzeResult> {
    let mut results = TiVec::new();

    for bb in func.bbs.iter_mut() {
        let mut defined = FxHashSet::default();
        let mut used = FxHashSet::default();

        if bb.id == func.bb_entry {
            for i in 0..func.args {
                defined.insert(LocalId::from(i));
            }
        }

        defined.insert(func.ret);

        bb.modify_local_id(|local_id, flag| match flag {
            LocalFlag::Defined => {
                defined.insert(*local_id);
            }
            LocalFlag::Used => {
                used.insert(*local_id);
            }
        });

        results.push(AnalyzeResult {
            defined_locals: defined,
            used_locals: used,
        });
    }

    // 推移的に使用を集計
    // BBは前方ジャンプがないことを仮定している
    let mut queue = VecDeque::new();
    let mut bb_ids = Vec::new();
    let mut visited = {
        let mut v = TiVec::with_capacity(func.bbs.len());
        v.resize(func.bbs.len(), false);
        v
    };
    queue.push_back(func.bb_entry);
    while let Some(bb_id) = queue.pop_front() {
        if visited[bb_id] {
            continue;
        }
        bb_ids.push(bb_id);
        visited[bb_id] = true;

        for succ in func.bbs[bb_id].next.successors() {
            queue.push_back(succ);
        }
    }

    // defineの集計は前から行う
    // 自分より前のブロックで定義済みの関数
    let mut prev_defines = TiVec::new();
    for _ in bb_ids.iter() {
        prev_defines.push(FxHashSet::<LocalId>::default());
    }
    for &bb_id in bb_ids.iter() {
        let result = &mut results[bb_id];
        for prev_define in prev_defines[bb_id].iter() {
            let removed = result.defined_locals.remove(prev_define);
            if removed {
                result.used_locals.insert(*prev_define);
            }
        }

        let prev_define = prev_defines[bb_id].clone();
        for succ in func.bbs[bb_id].next.successors() {
            prev_defines[succ].extend(&result.defined_locals);
            prev_defines[succ].extend(&prev_define);
        }
    }

    for &bb_id in bb_ids.iter().rev() {
        let mut result = results[bb_id].clone();

        for succ in func.bbs[bb_id].next.successors() {
            let succ_result = &results[succ];
            result.used_locals.extend(&succ_result.used_locals);
        }

        for define in &result.defined_locals {
            result.used_locals.remove(define);
        }

        results[bb_id] = result;
    }

    // エントリーポイントの例外的処理
    results[func.bb_entry].used_locals = (0..func.args)
        .map(|i| LocalId::from(i))
        .collect::<FxHashSet<_>>();
    for i in 0..func.args {
        results[func.bb_entry]
            .defined_locals
            .remove(&LocalId::from(i));
    }

    results
}

#[derive(Debug, Clone, Default)]
struct BBInfo {
    // argsとdefinesはorig func_id
    args: Vec<LocalId>,
    defines: Vec<LocalId>,
    // orig func_id -> new func_id
    locals_mapping: FxHashMap<LocalId, LocalId>,
}

fn calculate_bb_info(
    analyze_results: TiVec<BasicBlockId, AnalyzeResult>,
) -> TiVec<BasicBlockId, BBInfo> {
    let mut bb_info = TiVec::new();

    for result in analyze_results.into_iter() {
        let mut info = BBInfo {
            args: result.used_locals.into_iter().collect(),
            defines: result.defined_locals.into_iter().collect(),
            locals_mapping: FxHashMap::default(),
        };

        let mut local_id = LocalId::from(0);
        for &arg in &info.args {
            info.locals_mapping.insert(arg, local_id);
            local_id = LocalId::from(usize::from(local_id) + 1);
        }
        for &define in &info.defines {
            info.locals_mapping.insert(define, local_id);
            local_id = LocalId::from(usize::from(local_id) + 1);
        }

        bb_info.push(info);
    }

    bb_info
}

fn calculate_args_to_pass(caller: &BBInfo, callee: &BBInfo) -> Vec<LocalId> {
    let mut args_to_pass = Vec::new();
    for &arg in &callee.args {
        args_to_pass.push(caller.locals_mapping[&arg]);
    }
    args_to_pass
}

// module内の関数を1bb=1funcに分割する
pub fn split_function(mut module: Module) -> Module {
    let bb_infos = module
        .funcs
        .iter_mut()
        .map(|func| calculate_bb_info(analyze_locals(func)))
        .collect::<TiVec<FuncId, _>>();

    let (new_func_ids, bb_to_func_id) = {
        let mut new_func_ids = FxHashMap::default();
        let mut bb_to_func_id = FxHashMap::default();

        let mut func_id = FuncId::from(0);
        for orig_func in module.funcs.iter() {
            let new_func_id = func_id;
            new_func_ids.insert(orig_func.id, new_func_id);
            func_id = FuncId::from(usize::from(func_id) + orig_func.bbs.len());

            for bb in orig_func.bbs.iter() {
                let new_func_id = FuncId::from(usize::from(new_func_id) + usize::from(bb.id));
                bb_to_func_id.insert((orig_func.id, bb.id), new_func_id);
            }
        }

        (new_func_ids, bb_to_func_id)
    };

    let mut new_funcs = TiVec::new();

    for orig_func in module.funcs.into_iter() {
        for mut bb in orig_func.bbs.into_iter() {
            let new_func_id = bb_to_func_id[&(orig_func.id, bb.id)];

            let bb_info = &bb_infos[orig_func.id][bb.id];
            let mut new_locals = TiVec::new();

            for &arg in &bb_info.args {
                new_locals.push(orig_func.locals[arg]);
            }

            for &define in &bb_info.defines {
                new_locals.push(orig_func.locals[define]);
            }

            let new_ret = bb_info.locals_mapping[&orig_func.ret];

            bb.modify_local_id(|local_id, _| {
                *local_id = bb_info.locals_mapping[local_id];
            });
            for expr in bb.exprs.iter_mut() {
                expr.expr.modify_func_id(|func_id| {
                    let new_target_func_id = new_func_ids[func_id];
                    *func_id = new_target_func_id;
                });
            }

            let mut extra_bbs = Vec::new();

            let new_next = match bb.next {
                BasicBlockNext::If(cond, then_bb, else_bb) => {
                    let then_func_id = bb_to_func_id[&(orig_func.id, then_bb)];
                    let else_func_id = bb_to_func_id[&(orig_func.id, else_bb)];

                    let then_locals_to_pass =
                        calculate_args_to_pass(bb_info, &bb_infos[orig_func.id][then_bb]);
                    let else_locals_to_pass =
                        calculate_args_to_pass(bb_info, &bb_infos[orig_func.id][else_bb]);

                    let then_bb_new = BasicBlock {
                        id: BasicBlockId::from(1),
                        exprs: vec![ExprAssign {
                            local: Some(new_ret),
                            expr: Expr::Call(true, then_func_id, then_locals_to_pass),
                        }],
                        next: BasicBlockNext::Return,
                    };

                    let else_bb_new = BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![ExprAssign {
                            local: Some(new_ret),
                            expr: Expr::Call(true, else_func_id, else_locals_to_pass),
                        }],
                        next: BasicBlockNext::Return,
                    };

                    extra_bbs.push(then_bb_new);
                    extra_bbs.push(else_bb_new);

                    BasicBlockNext::If(cond, BasicBlockId::from(1), BasicBlockId::from(2))
                }
                BasicBlockNext::Jump(target_bb) => {
                    let target_func_id = bb_to_func_id[&(orig_func.id, target_bb)];
                    let args_to_pass =
                        calculate_args_to_pass(bb_info, &bb_infos[orig_func.id][target_bb]);

                    bb.exprs.push(ExprAssign {
                        local: Some(new_ret),
                        expr: Expr::Call(true, target_func_id, args_to_pass),
                    });

                    BasicBlockNext::Return
                }
                BasicBlockNext::Return => BasicBlockNext::Return,
            };

            let new_bb = BasicBlock {
                id: BasicBlockId::from(0),
                exprs: bb.exprs,
                next: new_next,
            };

            let mut new_bbs = TiVec::new();
            new_bbs.push(new_bb);
            new_bbs.extend(extra_bbs.into_iter());

            let new_func = Func {
                id: new_func_id,
                locals: new_locals,
                args: bb_info.args.len(),
                ret: new_ret,
                bb_entry: BasicBlockId::from(0),
                bbs: new_bbs,
            };

            new_funcs.push(new_func);
        }
    }

    let new_entry = new_func_ids[&module.entry];

    Module {
        globals: module.globals,
        funcs: new_funcs,
        entry: new_entry,
        meta: module.meta,
    }
}
