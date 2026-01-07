use rustc_hash::FxHashMap;
use vec_map::VecMap;
use webschembly_compiler_ir::*;

use crate::ir_processor::{
    cfg_analyzer::calculate_rpo,
    dataflow::{analyze_liveness, calc_def_use},
};

pub fn register_allocation(func: &mut Func) {
    // リニアスキャンレジスタアロケーションの実装
    // 1. 各変数の生存区間(Live Interval)を計算
    // 2. 生存区間を元に、同時の生存しない変数を同じLocalIdに割り当てることで変数を削減する
    
    // 生存区間の計算
    let intervals = build_intervals(func);
    
    // 型ごとにレジスタ割り当てを行う
    let mut new_local_map = FxHashMap::default();
    let mut new_locals = VecMap::new();

    // 型ごとにIntervalをグループ化
    // LocalTypeをキーにする
    let mut intervals_by_type: FxHashMap<LocalType, Vec<Interval>> = FxHashMap::default();
    
    for interval in intervals {
        let ty = func.locals[interval.local].typ.clone();
        intervals_by_type.entry(ty).or_default().push(interval);
    }
    
    for (ty, mut intervals) in intervals_by_type {
        // startの昇順にソート
        intervals.sort_by(|a, b| a.start.cmp(&b.start));
        
        let mut active: Vec<Interval> = Vec::new();
        let mut free_registers: Vec<usize> = Vec::new();
        let mut register_count = 0;
        
        // ローカルID -> 割り当てられたレジスタ番号
        let mut allocation: FxHashMap<LocalId, usize> = FxHashMap::default();
        
        for interval in intervals {
            // Expire Old Intervals
            let start_pos = interval.start;
            
            let mut i = 0;
            while i < active.len() {
                if active[i].end < start_pos {
                    let reg = allocation[&active[i].local];
                    free_registers.push(reg);
                    active.swap_remove(i);
                } else {
                    i += 1;
                }
            }
            
            // Allocate
            let reg = if let Some(reg) = free_registers.pop() {
                reg
            } else {
                let reg = register_count;
                register_count += 1;
                reg
            };
            
            allocation.insert(interval.local, reg);
            active.push(interval);
        }
        
        // レジスタ番号から新しいLocalIdへのマッピングを作成
        let mut register_to_local_id: FxHashMap<usize, LocalId> = FxHashMap::default();
        
        for (local, reg) in allocation {
            let new_id = *register_to_local_id.entry(reg).or_insert_with(|| {
                new_locals.push_with(|id| Local {
                    id,
                    typ: ty.clone(),
                })
            });
            new_local_map.insert(local, new_id);
        }
    }
    
    // argsの更新
    // 関数のシグネチャの一部なので、元の引数がどうマップされたかを確認し、
    // マップされていなければ新しく作る（unused argumentの場合）
    // NOTE: new_locals replaces func.locals.
    // We must ensure all args have a corresponding local in new_locals.
    
    for arg in &mut func.args {
        if let Some(&new_id) = new_local_map.get(arg) {
            *arg = new_id;
        } else {
             // 未使用の引数もLocalとして存在させる必要がある
             let ty = func.locals[*arg].typ.clone();
             let new_id = new_locals.push_with(|id| Local { id, typ: ty });
             new_local_map.insert(*arg, new_id);
             *arg = new_id;
        }
    }

    // 古いlocalsを置き換え
    func.locals = new_locals;
    
    // 命令の書き換え
    for bb in func.bbs.values_mut() {
        for instr in &mut bb.instrs {
            // 定義の書き換え
            if let Some(local) = instr.local {
                if let Some(&new_id) = new_local_map.get(&local) {
                    instr.local = Some(new_id);
                } else {
                    instr.local = None; 
                }
            }
            
            // 使用の書き換え
            for (local, flag) in instr.local_usages_mut() {
                if let LocalFlag::Used(_) = flag {
                    if let Some(&new_id) = new_local_map.get(local) {
                        *local = new_id;
                    } else {
                        // Unreachable or logic error?
                        // If it's used, it should have been in intervals and allocated.
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Interval {
    local: LocalId,
    start: usize,
    end: usize,
}

fn build_intervals(func: &Func) -> Vec<Interval> {
    // 1. ブロックをリニアライズ (RPO)
    let rpo = calculate_rpo(&func.bbs, func.bb_entry);
    let mut blocks = func.bbs.keys().collect::<Vec<_>>();
    blocks.sort_by_key(|id| rpo.get(id).unwrap_or(&usize::MAX));
    
    // 2. 命令に番号を割り振る
    let mut instr_indices = FxHashMap::default();
    let mut current_pos = 0;
    let mut block_ranges = FxHashMap::default(); // BlockId -> (Start, End)

    for &bb_id in &blocks {
        let start = current_pos;
        for (idx, _) in func.bbs[bb_id].instrs.iter().enumerate() {
            instr_indices.insert((bb_id, idx), current_pos);
            current_pos += 2; 
        }
        let end = current_pos; // ブロックの終わり
        block_ranges.insert(bb_id, (start, end));
    }
    
    // Dataflow Analysis
    let mut def_use = calc_def_use(&func.bbs);
    
    // 引数をEntryブロックのDefとして追加
    if let Some(entry_def_use) = def_use.get_mut(&func.bb_entry) {
        for &arg in &func.args {
            entry_def_use.defs.insert(arg);
        }
    }
    
    let liveness = analyze_liveness(&func.bbs, &def_use, &rpo);
    
    let mut interval_map: FxHashMap<LocalId, Interval> = FxHashMap::default();
    
    // initialize intervals for all locals (including args)
    for local_id in func.locals.keys() {
        interval_map.insert(local_id, Interval {
            local: local_id,
            start: usize::MAX,
            end: 0,
        });
    }
    
    // 引数のstartを0にする
    for &arg in &func.args {
        if let Some(interval) = interval_map.get_mut(&arg) {
            interval.start = 0;
        }
    }
    
    // Compute Intervals
    for &bb_id in blocks.iter().rev() {
        let (blk_start, blk_end) = block_ranges[&bb_id];
        let live_out = &liveness.live_out[&bb_id];
        
        for &local in live_out {
            if let Some(interval) = interval_map.get_mut(&local) {
                interval.end = std::cmp::max(interval.end, blk_end);
                interval.start = std::cmp::min(interval.start, blk_start); 
            }
        }
        
        let bb = &func.bbs[bb_id];
        for (idx, instr) in bb.instrs.iter().enumerate().rev() {
            let pos = instr_indices[&(bb_id, idx)];
            
            // Output (Def)
            if let Some(local) = instr.local {
                if let Some(interval) = interval_map.get_mut(&local) {
                    interval.start = pos + 1; // Allow reuse if use ends at pos
                }
            }
            
            // Input (Use)
            for (local, flag) in instr.local_usages() {
                if let LocalFlag::Used(_) = flag {
                    if let Some(interval) = interval_map.get_mut(&local) {
                        interval.end = std::cmp::max(interval.end, pos);
                        interval.start = std::cmp::min(interval.start, blk_start);
                    }
                }
            }
        }
    }
    
    interval_map.into_values()
        .filter(|i| i.start != usize::MAX)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vec_map::VecMap;

    #[test]
    fn test_register_allocation_reuse() {
        let mut locals = VecMap::new();
        // Assuming LocalType::Val(ValType::Int) or similar.
        // If Type::Val exists, maybe LocalType just wraps it, or is an alias?
        // Let's rely on Into conversion used in module_generator: Type::Val(...).into()
        // Or construct LocalType directly if possible.
        // We will try Type::Val(...).into() as it is proven in module_generator.
        // But Type is usually a struct/enum in IR. 
        // webschembly_compiler_ir::Type and webschembly_compiler_ir::ValType.
        
        let int_type: LocalType = Type::Val(ValType::Int).into();
        
        let v0 = locals.push_with(|id| Local { id, typ: int_type.clone() });
        let v1 = locals.push_with(|id| Local { id, typ: int_type.clone() });
        let v2 = locals.push_with(|id| Local { id, typ: int_type.clone() });
        let v3 = locals.push_with(|id| Local { id, typ: int_type.clone() });
        let v4 = locals.push_with(|id| Local { id, typ: int_type.clone() }); 
        
        let instr0 = Instr {
            local: Some(v2),
            kind: InstrKind::AddInt(v0, v1),
        };
        let instr1 = Instr {
            local: Some(v3),
            kind: InstrKind::Int(30),
        };
        let instr2 = Instr {
            local: Some(v4),
            kind: InstrKind::AddInt(v2, v3),
        };
        let terminator = TerminatorInstr::Exit(ExitInstr::Return(v4));
        
        let mut bbs = VecMap::new();
        let bb_entry = bbs.push_with(|id| BasicBlock {
            id,
            instrs: vec![instr0, instr1, instr2, Instr { local: None, kind: InstrKind::Terminator(terminator) }],
        });
        
        let mut func = Func {
            id: FuncId::from(0),
            bb_entry,
            locals,
            ret_type: int_type.clone(),
            args: vec![v0, v1],
            bbs,
            closure_meta: None,
        };
        
        assert_eq!(func.locals.keys().count(), 5);
        
        register_allocation(&mut func);
        
        let local_count = func.locals.keys().count();
        assert!(local_count < 5, "Locals should be reduced, got {}", local_count);
    }
}
