use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir_processor::ssa::DefUseChain;
use vec_map::VecMap;
use webschembly_compiler_ir::*;

pub fn inlining(module: &mut Module, module_inliner: &mut ModuleInliner, last: bool) {
    let mut funcs = VecMap::new();
    let mut func_analyze_results = FxHashMap::default();
    for func in module.funcs.values() {
        func_analyze_results.insert(func.id, FuncAnalyzeResult::new(func));
    }

    for func_id in module.funcs.keys() {
        let func = inlining_func(
            &func_analyze_results,
            module,
            func_id,
            module_inliner.func_inliners.get_mut(&func_id).unwrap(),
            last,
        );
        funcs.insert_node(func);
    }

    module.funcs = funcs;
}

fn inlining_func(
    func_analyze_results: &FxHashMap<FuncId, FuncAnalyzeResult>,
    module: &Module,
    func_id: FuncId,
    func_inliner: &mut FuncInliner,
    last: bool,
) -> Func {
    // インライン展開対象の関数が再帰的に依存している関数も含む
    // 自信を呼び出している箇所がないなら、自身を含まない
    let mut required_func_ids = {
        let mut required_func_ids = FxHashSet::default();
        let mut worklist = vec![func_id];
        while let Some(current_func_id) = worklist.pop() {
            for required_func in func_analyze_results[&current_func_id].call_funcs.values() {
                if required_func_ids.insert(required_func.func_id) {
                    worklist.push(required_func.func_id);
                }
            }
        }
        required_func_ids
    };

    if required_func_ids.is_empty() && func_inliner.merge_func_infos.is_empty()
    // TODO: matmul-64.b.scm で無限ループする
    {
        // required_func_ids={} なら自身をそのまま返すべき
        return module.funcs[func_id].clone();
    }

    required_func_ids.insert(func_id);

    let mut bbs = module.funcs[func_id].bbs.to_empty(); // to_emptyを使うのは遅い
    let mut locals = VecMap::<LocalId, Local>::new();

    // マージ済み関数の復元
    for merge_func_info in func_inliner.merge_func_infos.values() {
        for &local in merge_func_info.local_map.values() {
            locals.insert_node(module.funcs[func_id].locals[local]);
        }
        for &bb in merge_func_info.bb_map.values() {
            // remove_unreachable_bbで消されている可能性がある
            if let Some(bb) = module.funcs[func_id].bbs.get(bb) {
                bbs.insert_node(bb.clone());
            }
        }
    }

    let mut new_merge_func_ids = FxHashSet::default();

    for &required_func_id in &required_func_ids {
        if func_inliner
            .merge_func_infos
            .contains_key(&required_func_id)
        {
            continue;
        }

        new_merge_func_ids.insert(required_func_id);

        let mut local_map = FxHashMap::default();
        for (local_id, &local) in module.funcs[required_func_id].locals.iter() {
            let new_local_id = locals.push_with(|new_local_id| Local {
                id: new_local_id,
                ..local
            });
            local_map.insert(local_id, new_local_id);
        }
        let mut bb_map = FxHashMap::default();
        for bb_id in module.funcs[required_func_id].bbs.keys() {
            let new_bb_id = bbs.allocate_key();
            bb_map.insert(bb_id, new_bb_id);
        }

        for (bb_id, bb) in module.funcs[required_func_id].bbs.iter() {
            let mut new_bb = BasicBlock {
                id: bb_map[&bb_id],
                ..bb.clone()
            };
            for instr in &mut new_bb.instrs {
                for bb_id in instr.kind.bb_ids_mut() {
                    *bb_id = bb_map[bb_id];
                }
            }
            for (local_id, _) in new_bb.local_usages_mut() {
                *local_id = local_map[local_id];
            }

            bbs.insert_node(new_bb);
        }

        func_inliner.merge_func_infos.insert(
            required_func_id,
            MergeFuncInfo {
                args_phi_bb: bbs.allocate_key(),
                args_phi_incomings: module.funcs[required_func_id]
                    .args
                    .iter()
                    .map(|arg| ArgInfo {
                        local: local_map[arg],
                        incomings: Vec::new(),
                    })
                    .collect(),
                entry_bb_id: bb_map[&module.funcs[required_func_id].bb_entry],
                local_map,
                bb_map,
            },
        );
    }

    for &required_func_id in &required_func_ids {
        let func_analyze_result = &func_analyze_results[&required_func_id];

        for (bb_id, call) in &func_analyze_result.call_funcs {
            let merge_func_info = func_inliner
                .merge_func_infos
                .get_mut(&required_func_id)
                .unwrap();

            let new_bb_id = if new_merge_func_ids.contains(&required_func_id) {
                merge_func_info.bb_map[bb_id]
            } else {
                *bb_id
            };

            let args = if new_merge_func_ids.contains(&required_func_id) {
                call.args
                    .iter()
                    .map(|arg| merge_func_info.local_map[arg])
                    .collect::<Vec<_>>()
            } else {
                call.args.clone()
            };

            let args_phi_bb = func_inliner
                .merge_func_infos
                .get(&call.func_id)
                .unwrap()
                .args_phi_bb;

            let new_bb = &mut bbs[new_bb_id];
            debug_assert!(matches!(
                new_bb.terminator(),
                TerminatorInstr::Exit(ExitInstr::TailCallClosure(_))
            ));
            *new_bb.terminator_mut() = TerminatorInstr::Jump(args_phi_bb);

            for (i, &arg) in args.iter().enumerate() {
                func_inliner
                    .merge_func_infos
                    .get_mut(&call.func_id)
                    .unwrap()
                    .args_phi_incomings[i]
                    .incomings
                    .push(PhiIncomingValue {
                        local: arg,
                        bb: new_bb_id,
                    });
            }
        }
    }

    // 関数全体の引数を用意
    // 関数の引数の仮想的な生成場所であるBBを追加し、そのbb_idから引数を受け取るようなphiノードを追加
    let entry_bb_id = if let Some(entry_bb_id) = func_inliner.entry_bb_id {
        bbs.insert_node(module.funcs[func_id].bbs[entry_bb_id].clone());
        entry_bb_id
    } else {
        let entry_bb_id = bbs.push_with(|entry_bb_id| BasicBlock {
            id: entry_bb_id,
            instrs: vec![Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Jump(
                    func_inliner.merge_func_infos[&func_id].args_phi_bb,
                )),
            }],
        });

        func_inliner.entry_bb_id = Some(entry_bb_id);
        entry_bb_id
    };

    let args = if let Some(args) = &func_inliner.args {
        for arg in args {
            locals.insert_node(module.funcs[func_id].locals[*arg]);
        }

        args.clone()
    } else {
        let mut args = Vec::new();
        for arg_local in &module.funcs[func_id].args {
            let new_local_id = locals.push_with(|new_local_id| Local {
                id: new_local_id,
                ..module.funcs[func_id].locals[*arg_local]
            });
            args.push(new_local_id);
        }

        let args_phi_incomings = &mut func_inliner
            .merge_func_infos
            .get_mut(&func_id)
            .unwrap()
            .args_phi_incomings;
        for (i, &arg) in args.iter().enumerate() {
            args_phi_incomings[i].incomings.push(PhiIncomingValue {
                local: arg,
                bb: entry_bb_id,
            });
        }

        func_inliner.args = Some(args.clone());
        args
    };

    for merge_func_info in func_inliner.merge_func_infos.values() {
        let mut instrs = merge_func_info
            .args_phi_incomings
            .iter()
            .enumerate()
            .map(|(i, arg_info)| Instr {
                local: Some(arg_info.local),
                // i == 0は必ずクロージャであり、毎回引数は変わらない(本当？)ので、non_exhaustive=falseにしてよい
                kind: InstrKind::Phi {
                    incomings: arg_info.incomings.clone(),
                    non_exhaustive: i != 0 && !last,
                },
            })
            .collect::<Vec<_>>();

        instrs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(TerminatorInstr::Jump(merge_func_info.entry_bb_id)),
        });

        bbs.insert_node(BasicBlock {
            id: merge_func_info.args_phi_bb,
            instrs,
        });
    }

    Func {
        id: func_id,
        bb_entry: entry_bb_id,
        locals,
        ret_type: module.funcs[func_id].ret_type,
        args,
        bbs,
        closure_meta: module.funcs[func_id].closure_meta.clone(),
    }
}

#[derive(Debug, Clone)]
struct FuncAnalyzeResult {
    // あるBBの末尾がCallClosureかつ、FuncIdを静的に特定できる場合のId
    call_funcs: FxHashMap<BasicBlockId, InstrCall>,
}

impl FuncAnalyzeResult {
    fn new(func: &Func) -> Self {
        let def_use = DefUseChain::from_bbs(&func.bbs);
        let mut call_funcs = FxHashMap::default();
        for bb_id in func.bbs.keys() {
            if let TerminatorInstr::Exit(ExitInstr::TailCallClosure(
                    call_closure,
                )) = func.bbs[bb_id].terminator()
                    // TODO: func_idはそのモジュール内にあるとは限らない
                    && let Some(InstrKind::Closure {
                        func_id: call_func_id,
                        ..
                    }) = def_use.get_def_non_move_expr(&func.bbs, call_closure.closure)
            {
                // TODO: ここでdesugerのようなことをするのはあまりきれいではない
                call_funcs.insert(
                    bb_id,
                    InstrCall {
                        func_id: FuncId::from(*call_func_id),
                        args: {
                            let mut args = Vec::new();
                            args.push(call_closure.closure);
                            args.extend(&call_closure.args);
                            args
                        },
                    },
                );
            }
        }
        FuncAnalyzeResult { call_funcs }
    }
}

#[derive(Debug, Clone)]
struct MergeFuncInfo {
    // argsのphiノード用
    args_phi_bb: BasicBlockId,
    // エントリーBB
    entry_bb_id: BasicBlockId,
    args_phi_incomings: Vec<ArgInfo>,
    local_map: FxHashMap<LocalId, LocalId>,
    bb_map: FxHashMap<BasicBlockId, BasicBlockId>,
}

#[derive(Debug, Clone)]
struct ArgInfo {
    local: LocalId,
    incomings: Vec<PhiIncomingValue>,
}

#[derive(Debug, Clone, Default)]
struct FuncInliner {
    merge_func_infos: FxHashMap<FuncId, MergeFuncInfo>,
    entry_bb_id: Option<BasicBlockId>,
    args: Option<Vec<LocalId>>,
}

#[derive(Debug, Clone)]
pub struct ModuleInliner {
    func_inliners: FxHashMap<FuncId, FuncInliner>,
}

impl ModuleInliner {
    pub fn new(module: &Module) -> Self {
        let mut func_inliners = FxHashMap::default();
        for func_id in module.funcs.keys() {
            func_inliners.insert(func_id, FuncInliner::default());
        }
        ModuleInliner { func_inliners }
    }
}
