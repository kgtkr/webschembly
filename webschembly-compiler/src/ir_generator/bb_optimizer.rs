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
    locals_immutability: &mut VecMap<LocalId, bool>,
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
            locals_immutability.push(true); // obj_localは再代入されない
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

pub fn analyze_typed_obj(
    bb: &BasicBlock,
    locals_immutability: &VecMap<LocalId, bool>,
) -> VecMap<LocalId, TypedObj> {
    // 次のBBに引き継ぐ型情報
    let mut typed_objs = VecMap::new();

    for expr_assign in bb.exprs.iter() {
        match *expr_assign {
            ExprAssign {
                local: Some(local),
                expr: Expr::FromObj(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
                typed_objs.insert(value, TypedObj {
                    val_type: local,
                    typ,
                });
            }
            ExprAssign {
                local: Some(local),
                expr: Expr::ToObj(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
                typed_objs.insert(local, TypedObj {
                    val_type: value,
                    typ,
                });
            }
            _ => {}
        }
    }

    typed_objs
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedObj {
    pub val_type: LocalId,
    pub typ: ValType,
}

// 変数の不変判定
pub fn analyze_locals_immutability(
    locals: &VecMap<LocalId, Local>,
    bb: &BasicBlock,
    args: &Vec<LocalId>,
) -> VecMap<LocalId, bool> {
    let mut assign_counts = VecMap::new();
    for local in locals.keys() {
        assign_counts.insert(local, 0);
    }
    for &arg in args {
        assign_counts[arg] += 1;
    }
    for expr_assign in &bb.exprs {
        if let Some(local) = expr_assign.local {
            assign_counts[local] += 1;
        }
    }

    assign_counts
        .into_iter()
        .map(|(id, count)| (id, count <= 1))
        .collect::<VecMap<LocalId, bool>>()
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
pub fn copy_propagate(
    locals: &VecMap<LocalId, Local>,
    bb: &mut BasicBlock,
    locals_immutability: &VecMap<LocalId, bool>,
) {
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
            if flag == LocalFlag::Used {
                *local = local_replacements[*local];
            }
        }

        match *expr_assign {
            ExprAssign {
                local: Some(local),
                expr: Move(value),
            } if locals_immutability[value] && locals_immutability[local] => {
                local_replacements[local] = value;
            }
            ExprAssign {
                local: Some(local),
                expr: ToObj(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
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
            } if locals_immutability[value] && locals_immutability[local] => {
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
    _locals_immutability: &VecMap<LocalId, bool>,
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
                if flag == LocalFlag::Used {
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
    use crate::fxbihashmap::FxBiHashMap;
    use typed_index_collections::ti_vec;

    #[test]
    fn test_analyze_locals_immutability() {
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
            Local {
                id: LocalId::from(3),
                typ: LocalType::Type(Type::Val(ValType::Int)),
            },
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        let bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: Some(LocalId::from(0)),
                    expr: Expr::Move(LocalId::from(1)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Move(LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Int(0),
                },
                ExprAssign {
                    local: Some(LocalId::from(3)),
                    expr: Expr::Int(0),
                },
            ],
            next: BasicBlockNext::Terminator(BasicBlockTerminator::Return(LocalId::from(1))),
        };

        let args = vec![LocalId::from(0), LocalId::from(1)];

        let immutability = analyze_locals_immutability(&locals, &bb, &args);

        assert_eq!(
            immutability,
            [
                (LocalId::from(0), false),
                (LocalId::from(1), true),
                (LocalId::from(2), false),
                (LocalId::from(3), true),
            ]
            .into_iter()
            .collect::<VecMap<_, _>>()
        );
    }

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
        let mut locals_immutability = [
            (LocalId::from(0), true),
            (LocalId::from(1), true),
            (LocalId::from(2), true),
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        let assigned_local_to_obj = assign_type_args(
            &mut locals,
            &mut bb,
            &type_params,
            &type_args,
            &mut locals_immutability,
        );

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
            locals_immutability,
            [
                (LocalId::from(0), true),
                (LocalId::from(1), true),
                (LocalId::from(2), true),
                (LocalId::from(3), true),
            ]
            .into_iter()
            .collect::<VecMap<LocalId, _>>()
        );

        assert_eq!(
            assigned_local_to_obj,
            vec![(LocalId::from(0), LocalId::from(3)),]
                .into_iter()
                .collect::<VecMap<LocalId, _>>()
        );

        let typed_objs = analyze_typed_obj(&bb, &locals_immutability);

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

        let locals_immutability = [
            (LocalId::from(0), true),
            (LocalId::from(1), true),
            (LocalId::from(2), true),
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        copy_propagate(&locals, &mut bb, &locals_immutability);

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

        let locals_immutability = [
            (LocalId::from(0), true),
            (LocalId::from(1), true),
            (LocalId::from(2), true),
            (LocalId::from(3), true),
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();

        copy_propagate(&locals, &mut bb, &locals_immutability);

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

        let locals_immutability = [
            (LocalId::from(0), true),
            (LocalId::from(1), true),
            (LocalId::from(2), true),
        ]
        .into_iter()
        .collect::<VecMap<LocalId, _>>();
        let out_used_locals = vec![];

        dead_code_elimination(&locals, &mut bb, &locals_immutability, &out_used_locals);

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
