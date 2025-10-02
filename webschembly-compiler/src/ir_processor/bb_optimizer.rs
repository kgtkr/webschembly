use typed_index_collections::TiVec;

use crate::{VecMap, fxbihashmap::FxBiHashMap, ir::*};

/*
BBに型代入を行う

```
local l1: obj
local l2: int
local l3: int

args: l1

_ = cons(l1, l1)
l2 = from_obj<int>(l1)
l3 = from_obj<int>(l1)
_ = add(l2, l3)
```

`l1 = int` を代入した場合:

```
local l1: int // intに更新
local l2: int
local l3: int
local l1_obj: obj // objバージョンを追加

args: l1

l1_obj = to_obj<int>(l1) // BBの先頭に追加。不要なら後々の最適化で削除する
_ = cons(l1_obj, l1_obj) // l1を参照している式はl1_objに置き換える
l2 = from_obj<int>(l1_obj)
l3 = from_obj<int>(l1_obj)
_ = add(l2, l3)
```
*/

pub fn assign_type_args(
    locals: &mut VecMap<LocalId, Local>,
    bb: &mut BasicBlock,
    type_params: &FxBiHashMap<TypeParamId, LocalId>,
    type_args: &TiVec<TypeParamId, Option<ValType>>,
) -> VecMap<LocalId, LocalId> {
    let mut additional_expr_assigns = Vec::new();

    // 型代入されている変数のobj版を用意(l1_objに対応)
    let mut assigned_local_to_obj = VecMap::new();

    for (type_param_id, typ) in type_args.iter_enumerated() {
        if let Some(typ) = typ {
            let local = *type_params.get_by_left(&type_param_id).unwrap();

            // ローカル変数の型に代入
            debug_assert_eq!(locals[local].typ, LocalType::Type(Type::Obj));
            locals[local].typ = LocalType::Type(Type::Val(*typ));

            // obj版のローカル変数を用意
            let obj_local = locals.push_with(|id| Local {
                id,
                typ: LocalType::Type(Type::Obj),
            });
            assigned_local_to_obj.insert(local, obj_local);
            additional_expr_assigns.push(ExprAssign {
                local: Some(obj_local),
                expr: Expr::ToObj(*typ, *type_params.get_by_left(&type_param_id).unwrap()),
            });
        }
    }

    for (local, _) in bb.local_usages_mut() {
        if let Some(&obj_local) = assigned_local_to_obj.get(*local) {
            *local = obj_local;
        }
    }

    bb.exprs.splice(0..0, additional_expr_assigns);

    assigned_local_to_obj
}

/*
静的に型が分かっているobj型の変数を収集する

```
local l1: int
local l2: int
local l3: int
local l1_obj: obj
local l4: cons
local l5: obj

args: l1

l1_obj = obj<int>(l1) // l1_objは中身がintであり、val_type版はl1にある
_ = cons(l1_obj, l1_obj)
l2 = from_obj<int>(l1_obj) // 1行目と同じ情報が得られる
l3 = from_obj<int>(l1_obj) // 同様
_ = add(l2, l3)
```

次のBBに引き継ぐ情報:
- objな値→val_typeな値の対応とその型(どちらも再代入されない場合のみ)
*/
// TODO: 前方/後方のmoveによる伝播に関するテスト追加
pub fn analyze_typed_obj(
    bb: &BasicBlock,
    defs: &VecMap<LocalId, usize>,
) -> VecMap<LocalId, TypedObj> {
    let mut typed_objs = VecMap::new();
    extend_typed_obj(bb, defs, &mut typed_objs);

    typed_objs
}

pub fn extend_typed_obj(
    bb: &BasicBlock,
    defs: &VecMap<LocalId, usize>,
    typed_objs: &mut VecMap<LocalId, TypedObj>,
) {
    let mut worklist = Vec::new();

    for expr_assign in bb.exprs.iter() {
        match *expr_assign {
            ExprAssign {
                local: Some(local),
                expr: Expr::FromObj(typ, value),
            } => {
                typed_objs.entry(value).or_insert(TypedObj {
                    val_type: local,
                    typ,
                });
                worklist.push(value);
            }
            ExprAssign {
                local: Some(local),
                expr: Expr::ToObj(typ, value),
            } => {
                typed_objs.entry(local).or_insert(TypedObj {
                    val_type: value,
                    typ,
                });
            }
            // 後方に型情報を伝播
            ExprAssign {
                local: Some(local),
                expr: Expr::Move(value),
            } => {
                if let Some(&typed_obj) = typed_objs.get(value) {
                    typed_objs.entry(local).or_insert(typed_obj);
                }
            }
            _ => {}
        }
    }

    // 前方のmoveをたどって型情報を伝播
    while let Some(local) = worklist.pop() {
        if let Some(def_idx) = defs.get(local) {
            if let Some(&ExprAssign {
                local: Some(_),
                expr: Expr::Move(value),
            }) = bb.exprs.get(*def_idx)
            {
                if !typed_objs.contains_key(value) {
                    let typed_obj = typed_objs[local];
                    typed_objs.entry(value).or_insert(typed_obj);
                    worklist.push(value);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedObj {
    pub val_type: LocalId,
    pub typ: ValType,
}

// TODO: テスト追加
pub fn remove_type_check(
    bb: &mut BasicBlock,
    typed_objs: &VecMap<LocalId, TypedObj>,
    defs: &VecMap<LocalId, usize>,
) {
    for (i, expr_assign) in bb.exprs.iter_mut().enumerate() {
        match expr_assign.expr {
            Expr::Is(val_type, local) => {
                if let Some(typed_obj) = typed_objs.get(local) {
                    if typed_obj.typ == val_type {
                        expr_assign.expr = Expr::Bool(true);
                    } else {
                        expr_assign.expr = Expr::Bool(false);
                    }
                }
            }
            Expr::FromObj(ty, obj_local) => {
                if let Some(typed_obj) = typed_objs.get(obj_local)
                    && defs
                        .get(typed_obj.val_type)
                        .map(|&def_idx| def_idx < i)
                        .unwrap_or(true)
                // defsに存在しない = 先行ブロックで定義されている
                {
                    debug_assert_eq!(typed_obj.typ, ty);
                    expr_assign.expr = Expr::Move(typed_obj.val_type);
                }
            }
            _ => {}
        }
    }
}

/*
コピー伝播を行う
対象はmove, to_obj-from_obj, from_obj-to_obj

例:

b = move a
c = f b
-->
b = move a
c = f a

b = to_obj<int> a
c = from_obj<int> b
d = g c
-->
b = to_obj<int> a
c = from_obj<int> a
d = g a

b = from_obj<int> a
c = to_obj<int> b
d = g c
-->
b = from_obj<int> a
c = to_obj<int> a
d = g a

ここでは、デッドコードの削除は行わない
*/
pub fn copy_propagate(locals: &VecMap<LocalId, Local>, bb: &mut BasicBlock) {
    // ローカル変数の置き換え情報
    let mut local_replacements = VecMap::new();
    // to_obj-from_objの置き換え情報
    let mut to_obj_replacements = VecMap::new();
    let mut from_obj_replacements = VecMap::new();
    for local in locals.keys() {
        local_replacements.insert(local, local);
        to_obj_replacements.insert(local, None);
        from_obj_replacements.insert(local, None);
    }

    for expr_assign in bb.exprs.iter_mut() {
        use Expr::*;

        for (local, flag) in expr_assign.local_usages_mut() {
            if let LocalFlag::Used(_) = flag {
                *local = local_replacements[*local];
            }
        }

        match *expr_assign {
            ExprAssign {
                local: Some(local),
                expr: Move(value),
            } => {
                local_replacements[local] = value;
            }
            ExprAssign {
                local: Some(local),
                expr: ToObj(typ, value),
            } => {
                to_obj_replacements[local] = Some((value, typ));

                if let Some((val_type, val_type_typ)) = from_obj_replacements[value]
                    && val_type_typ == typ
                // TODO: val_type_typ == typは必要？
                {
                    local_replacements[local] = val_type;
                }
            }
            ExprAssign {
                local: Some(local),
                expr: FromObj(typ, value),
            } => {
                from_obj_replacements[local] = Some((value, typ));

                if let Some((obj, obj_typ)) = to_obj_replacements[value]
                    && obj_typ == typ
                {
                    local_replacements[local] = obj;
                }
            }
            _ => {}
        };
    }

    for local in bb.next.local_ids_mut() {
        *local = local_replacements[*local]
    }
}

/*
デッドコード削除
*/
pub fn dead_code_elimination(
    locals: &VecMap<LocalId, Local>,
    bb: &mut BasicBlock,
    // 別のBBなどで使われているローカル変数
    out_used_locals: &Vec<LocalId>,
) {
    let mut used = VecMap::new();
    for local in locals.keys() {
        used.insert(local, false);
    }
    for &local in out_used_locals {
        used[local] = true;
    }
    for &local in bb.next.local_ids() {
        used[local] = true;
    }

    for expr_assign in bb.exprs.iter_mut().rev() {
        let is_effectful = expr_assign.expr.is_effectful();
        let expr_used = is_effectful || expr_assign.local.map(|l| used[l]).unwrap_or(false);
        if expr_used {
            for (&local, flag) in expr_assign.local_usages() {
                if let LocalFlag::Used(_) = flag {
                    used[local] = true;
                }
            }
        } else {
            expr_assign.expr = Expr::Nop;
            expr_assign.local = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fxbihashmap::FxBiHashMap, ir_processor::ssa::collect_defs};
    use typed_index_collections::ti_vec;

    // TODO: assign_type_argsとanalyze_typed_objのテストは分割したほうがいいかも？
    #[test]
    fn test_assign_type_args_and_analyze_typed_obj() {
        let mut locals = [
            Local {
                id: LocalId::from(0),
                typ: LocalType::Type(Type::Obj),
            },
            Local {
                id: LocalId::from(1),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(2),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: None,
                    expr: Expr::Cons(LocalId::from(0), LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::FromObj(ValType::Int, LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::FromObj(ValType::Int, LocalId::from(0)),
                },
                ExprAssign {
                    local: None,
                    expr: Expr::Add(LocalId::from(1), LocalId::from(2)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(1))),
        };

        let type_params = FxBiHashMap::from_iter(vec![(TypeParamId::from(0), LocalId::from(0))]);
        let type_args = ti_vec![Some(ValType::Int)];

        let assigned_local_to_obj =
            assign_type_args(&mut locals, &mut bb, &type_params, &type_args);

        assert_eq!(
            locals,
            [
                Local {
                    id: LocalId::from(0),
                    typ: LocalType::Type(Type::Val(ValType::Int)),
                },
                Local {
                    id: LocalId::from(1),
                    typ: LocalType::Type(Type::Val(ValType::Int)),
                },
                Local {
                    id: LocalId::from(2),
                    typ: LocalType::Type(Type::Val(ValType::Int)),
                },
                Local {
                    id: LocalId::from(3),
                    typ: LocalType::Type(Type::Obj),
                },
            ]
            .into_iter()
            .collect::<VecMap<LocalId, _>>()
        );

        assert_eq!(bb.exprs, vec![
            ExprAssign {
                local: Some(LocalId::from(3)),
                expr: Expr::ToObj(ValType::Int, LocalId::from(0)),
            },
            ExprAssign {
                local: None,
                expr: Expr::Cons(LocalId::from(3), LocalId::from(3)),
            },
            ExprAssign {
                local: Some(LocalId::from(1)),
                expr: Expr::FromObj(ValType::Int, LocalId::from(3)),
            },
            ExprAssign {
                local: Some(LocalId::from(2)),
                expr: Expr::FromObj(ValType::Int, LocalId::from(3)),
            },
            ExprAssign {
                local: None,
                expr: Expr::Add(LocalId::from(1), LocalId::from(2)),
            },
        ]);

        assert_eq!(
            assigned_local_to_obj,
            vec![(LocalId::from(0), LocalId::from(3)),]
                .into_iter()
                .collect::<VecMap<LocalId, _>>()
        );

        let defs = collect_defs(&bb);
        let typed_objs = analyze_typed_obj(&bb, &defs);

        assert_eq!(
            typed_objs,
            [(LocalId::from(3), TypedObj {
                val_type: LocalId::from(2),
                typ: ValType::Int
            })]
            .into_iter()
            .collect::<VecMap<LocalId, _>>()
        );
    }

    #[test]
    fn test_copy_propagate_move() {
        let locals = [
            Local {
                id: LocalId::from(0),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(1),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(2),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Move(LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Add(LocalId::from(1), LocalId::from(1)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(2))),
        };

        copy_propagate(&locals, &mut bb);

        assert_eq!(bb, BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Move(LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(2))),
        });
    }

    #[test]
    fn test_copy_propagate_to_obj_from_obj() {
        let locals = [
            Local {
                id: LocalId::from(0),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(1),
                typ: LocalType::Type(Type::Obj),
            },
            Local {
                id: LocalId::from(2),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(3),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::ToObj(ValType::Int, LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::FromObj(ValType::Int, LocalId::from(1)),
                },
                ExprAssign {
                    local: Some(LocalId::from(3)),
                    expr: Expr::Add(LocalId::from(2), LocalId::from(2)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(3))),
        };

        copy_propagate(&locals, &mut bb);

        assert_eq!(bb, BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::ToObj(ValType::Int, LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::FromObj(ValType::Int, LocalId::from(1)),
                },
                ExprAssign {
                    local: Some(LocalId::from(3)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(3))),
        });
    }

    #[test]
    fn test_dead_code_elimination() {
        let locals = [
            Local {
                id: LocalId::from(0),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(1),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
            Local {
                id: LocalId::from(2),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(2))),
        };

        let out_used_locals = vec![];

        dead_code_elimination(&locals, &mut bb, &out_used_locals);

        assert_eq!(bb, BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: None,
                    expr: Expr::Nop,
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(2))),
        });
    }
}
