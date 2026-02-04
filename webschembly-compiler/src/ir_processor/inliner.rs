use rustc_hash::FxHashMap;

use crate::ir_processor::ssa::DefUseChain;
use webschembly_compiler_ir::*;

pub fn inlining(module: &mut Module, module_inliner: &mut ModuleInliner, last: bool) {
    for func_id in module.funcs.keys().collect::<Vec<_>>() {
        inlining_func(
            module,
            func_id,
            module_inliner.func_inliners.get_mut(&func_id).unwrap(),
            last,
        );
    }
}

fn inlining_func(module: &mut Module, func_id: FuncId, func_inliner: &mut FuncInliner, last: bool) {
    // 無駄なクローン
    let mut func = module.funcs[func_id].clone();
    let analyze_result = FuncAnalyzeResult::new(&func);

    // 必要な関数のマージ
    for required_func_id in analyze_result.call_funcs.values().map(|c| c.func_id) {
        if func_inliner
            .merge_func_infos
            .contains_key(&required_func_id)
        {
            continue;
        }

        let mut local_map = FxHashMap::default();
        for (local_id, &local) in module.funcs[required_func_id].locals.iter() {
            let new_local_id = func.locals.push_with(|new_local_id| Local {
                id: new_local_id,
                ..local
            });
            local_map.insert(local_id, new_local_id);
        }
        let mut bb_map = FxHashMap::default();
        for bb_id in module.funcs[required_func_id].bbs.keys() {
            let new_bb_id = func.bbs.allocate_key();
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

            func.bbs.insert_node(new_bb);
        }

        func_inliner.merge_func_infos.insert(
            required_func_id,
            MergeFuncInfo {
                args_phi_bb: func.bbs.allocate_key(),
                args_phi_incomings: module.funcs[required_func_id]
                    .args
                    .iter()
                    .map(|arg| ArgInfo {
                        local: local_map[arg],
                        incomings: Vec::new(),
                    })
                    .collect(),
                entry_bb_id: bb_map[&module.funcs[required_func_id].bb_entry],
            },
        );
    }

    // CallClosure命令をジャンプ命令に書き換え
    for (&bb_id, call) in analyze_result.call_funcs.iter() {
        let args_phi_bb = func_inliner
            .merge_func_infos
            .get(&call.func_id)
            .unwrap()
            .args_phi_bb;

        let new_bb = &mut func.bbs[bb_id];
        debug_assert!(matches!(
            new_bb.terminator(),
            TerminatorInstr::Exit(ExitInstr::TailCallClosure(_))
        ));
        *new_bb.terminator_mut() = TerminatorInstr::Jump(args_phi_bb);

        for (i, &arg) in call.args.iter().enumerate() {
            func_inliner
                .merge_func_infos
                .get_mut(&call.func_id)
                .unwrap()
                .args_phi_incomings[i]
                .incomings
                .push(PhiIncomingValue {
                    local: arg,
                    bb: bb_id,
                });
        }
    }

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

        func.bbs.insert_node(BasicBlock {
            id: merge_func_info.args_phi_bb,
            instrs,
        });
    }

    module.funcs.insert_node(func);
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
}

#[derive(Debug, Clone)]
struct ArgInfo {
    local: LocalId,
    incomings: Vec<PhiIncomingValue>,
}

#[derive(Debug, Clone, Default)]
struct FuncInliner {
    merge_func_infos: FxHashMap<FuncId, MergeFuncInfo>,
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
