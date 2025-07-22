use rustc_hash::FxHashMap;

use crate::ir::*;
use typed_index_collections::TiVec;

// module内の関数を1bb=1funcに分割する
pub fn split_function(module: Module) -> Module {
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
        for bb in orig_func.bbs.into_iter() {
            let new_func_id = bb_to_func_id[&(orig_func.id, bb.id)];

            let new_locals = orig_func.locals.clone();

            let new_args = if bb.id == orig_func.bb_entry {
                // エントリポイントなら引数の数を保持
                orig_func.args
            } else {
                // エントリポイントでない場合、全てのローカル変数を引数で受け取る
                new_locals.len()
            };

            let new_ret = orig_func.ret;

            // FuncIdの更新
            let mut new_exprs = bb
                .exprs
                .into_iter()
                .map(|mut expr_assign| {
                    expr_assign.expr.modify_func_id(|func_id| {
                        let new_target_func_id = new_func_ids[func_id];
                        *func_id = new_target_func_id;
                    });
                    expr_assign
                })
                .collect::<Vec<_>>();

            let mut extra_bbs = Vec::new();
            let all_locals = orig_func.locals.keys().collect::<Vec<_>>();
            let new_next = match bb.next {
                BasicBlockNext::If(_cond, then_bb, else_bb) => {
                    // nextがIf: 3つのBBを持つ関数に変換

                    let then_func_id = bb_to_func_id[&(orig_func.id, then_bb)];
                    let else_func_id = bb_to_func_id[&(orig_func.id, else_bb)];

                    let then_bb_new = BasicBlock {
                        id: BasicBlockId::from(1),
                        exprs: vec![ExprAssign {
                            local: Some(new_ret),
                            expr: Expr::Call(true, then_func_id, all_locals.clone()),
                        }],
                        next: BasicBlockNext::Return,
                    };

                    let else_bb_new = BasicBlock {
                        id: BasicBlockId::from(2),
                        exprs: vec![ExprAssign {
                            local: Some(new_ret),
                            expr: Expr::Call(true, else_func_id, all_locals),
                        }],
                        next: BasicBlockNext::Return,
                    };

                    extra_bbs.push(then_bb_new);
                    extra_bbs.push(else_bb_new);

                    BasicBlockNext::If(_cond, BasicBlockId::from(1), BasicBlockId::from(2))
                }
                BasicBlockNext::Jump(target_bb) => {
                    let target_func_id = bb_to_func_id[&(orig_func.id, target_bb)];

                    new_exprs.push(ExprAssign {
                        local: Some(new_ret),
                        expr: Expr::Call(true, target_func_id, all_locals),
                    });

                    BasicBlockNext::Return
                }
                BasicBlockNext::Return => BasicBlockNext::Return,
            };

            let new_bb = BasicBlock {
                id: BasicBlockId::from(0),
                exprs: new_exprs,
                next: new_next,
            };

            let mut new_bbs = TiVec::new();
            new_bbs.push(new_bb);
            new_bbs.extend(extra_bbs.into_iter());

            let new_func = Func {
                id: new_func_id,
                locals: new_locals,
                args: new_args,
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
