use typed_index_collections::{TiVec, ti_vec};

use crate::{fxbihashmap::FxBiHashMap, ir::*};

/*
BBに型代入を行う

```
local l1: boxed
local l2: int
local l3: int

args: l1

_ = cons(l1, l1)
l2 = unbox<int>(l1)
l3 = unbox<int>(l1)
_ = add(l2, l3)
```

`l1 = int` を代入した場合:

```
local l1: int // intに更新
local l2: int
local l3: int
local l1_boxed: boxed // boxedバージョンを追加

args: l1

l1_boxed = boxed<int>(l1) // BBの先頭に追加。不要なら後々の最適化で削除する
_ = cons(l1_boxed, l1_boxed) // l1を参照している式はl1_boxedに置き換える
l2 = unbox<int>(l1_boxed)
l3 = unbox<int>(l1_boxed)
_ = add(l2, l3)
```
*/

pub fn assign_type_args(
    locals: &mut TiVec<LocalId, LocalType>,
    bb: &mut BasicBlock,
    type_params: &FxBiHashMap<TypeParamId, LocalId>,
    type_args: &TiVec<TypeParamId, Option<ValType>>,
    locals_immutability: &mut TiVec<LocalId, bool>,
) -> TiVec<LocalId, Option<LocalId>> {
    let mut additional_expr_assigns = Vec::new();

    // 型代入されている変数のboxed版を用意(l1_boxedに対応)
    let mut assigned_local_to_box = ti_vec![None; locals.len()];
    for (type_param_id, typ) in type_args.iter_enumerated() {
        if let Some(typ) = typ {
            let local = *type_params.get_by_left(&type_param_id).unwrap();

            // ローカル変数の型に代入
            debug_assert_eq!(locals[local], LocalType::Type(Type::Boxed));
            locals[local] = LocalType::Type(Type::Val(*typ));

            // boxed版のローカル変数を用意
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));
            assigned_local_to_box[local] = Some(boxed_local);
            additional_expr_assigns.push(ExprAssign {
                local: Some(boxed_local),
                expr: Expr::Box(*typ, *type_params.get_by_left(&type_param_id).unwrap()),
            });
            locals_immutability.push(true); // boxed_localは再代入されない
        }
    }

    for (local, _) in bb.local_usages_mut() {
        if let Some(boxed_local) = assigned_local_to_box[*local] {
            *local = boxed_local;
        }
    }

    bb.exprs.splice(0..0, additional_expr_assigns);

    assigned_local_to_box
}

/*
静的に型が分かっているboxed型の変数を収集する

```
local l1: int
local l2: int
local l3: int
local l1_boxed: boxed
local l4: cons
local l5: boxed

args: l1

l1_boxed = boxed<int>(l1) // l1_boxedは中身がintであり、unboxed版はl1にある
_ = cons(l1_boxed, l1_boxed)
l2 = unbox<int>(l1_boxed) // 1行目と同じ情報が得られる
l3 = unbox<int>(l1_boxed) // 同様
_ = add(l2, l3)
```

次のBBに引き継ぐ情報:
- boxedな値→unboxedな値の対応とその型(どちらも再代入されない場合のみ)
*/

pub fn analyze_typed_box(
    locals: &TiVec<LocalId, LocalType>,
    bb: &BasicBlock,
    locals_immutability: &TiVec<LocalId, bool>,
) -> TiVec<LocalId, Option<TypedBox>> {
    // 次のBBに引き継ぐ型情報
    let mut typed_boxes = ti_vec![None; locals.len()];

    for expr_assign in bb.exprs.iter() {
        match *expr_assign {
            ExprAssign {
                local: Some(local),
                expr: Expr::Unbox(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
                typed_boxes[value] = Some(TypedBox {
                    unboxed: local,
                    typ,
                });
            }
            ExprAssign {
                local: Some(local),
                expr: Expr::Box(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
                typed_boxes[local] = Some(TypedBox {
                    unboxed: value,
                    typ,
                });
            }
            _ => {}
        }
    }

    typed_boxes
}

/*
```
local l1: boxed
local l2: int
local l3: int

args: l1

_ = cons(l1, l1)
l2 = unbox<int>(l1)
l3 = unbox<int>(l1)
_ = add(l2, l3)
```

`l1 = int` を代入した場合:

```
local l1: int
local l2: int
local l3: int
local l1_boxed: boxed // boxedバージョンを追加

args: l1

l1_boxed = boxed<int>(l1) // BBの先頭に追加。不要なら後々の最適化で削除する
_ = cons(l1_boxed, l1_boxed) // l1を参照している式はl1_boxedに置き換える
l2 = unbox<int>(l1_boxed)
l3 = unbox<int>(l1_boxed)
_ = add(l1, l1)
```

次のBBに引き継ぐ情報:
- boxedな値→unboxedな値の対応とその型(どちらも再代入されない場合のみ)
*/

pub fn remove_box(
    locals: &mut TiVec<LocalId, LocalType>,
    bb: &mut BasicBlock,
    type_params: &FxBiHashMap<TypeParamId, LocalId>,
    type_args: &TiVec<TypeParamId, Option<ValType>>,
    locals_immutability: &mut TiVec<LocalId, bool>,
) -> TiVec<LocalId, Option<TypedBox>> {
    let mut additional_expr_assigns = Vec::new();

    // 型代入されている変数のboxed版を用意(l1_boxedに対応)
    let mut boxed_locals = ti_vec![None; locals.len()];
    for (type_param_id, typ) in type_args.iter_enumerated() {
        if let Some(typ) = typ {
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));
            boxed_locals[*type_params.get_by_left(&type_param_id).unwrap()] = Some(boxed_local);

            additional_expr_assigns.push(ExprAssign {
                local: Some(boxed_local),
                expr: Expr::Box(*typ, *type_params.get_by_left(&type_param_id).unwrap()),
            });
            locals_immutability.push(true); // boxed_localは再代入されない
        }
    }

    let boxed_locals = boxed_locals;

    // ローカル変数の置き換え情報
    let mut local_replacements = TiVec::new();
    for (from, to) in boxed_locals.iter_enumerated() {
        local_replacements.push(to.unwrap_or(from));
    }

    // 次のBBに引き継ぐ型情報
    let mut next_type_args = ti_vec![None; locals.len()];

    for expr_assign in bb.exprs.iter_mut() {
        if let ExprAssign {
            local,
            expr: Expr::Unbox(typ, value),
        } = *expr_assign
            && locals_immutability[value]
            && let Some(local) = local
            && locals_immutability[local]
        {
            next_type_args[value] = Some(TypedBox {
                unboxed: local,
                typ,
            });
        }

        for (local, _) in expr_assign.local_usages_mut() {
            *local = local_replacements[*local];
        }
    }

    for local in bb.next.local_ids_mut() {
        *local = local_replacements[*local]
    }

    for (unboxed, &boxed) in boxed_locals.iter_enumerated() {
        if let Some(boxed) = boxed
            && locals_immutability[unboxed]
            && locals_immutability[boxed]
        {
            let LocalType::Type(Type::Val(typ)) = locals[unboxed] else {
                unreachable!("expect val type, actual: {:?}", locals[unboxed]);
            };
            next_type_args[boxed] = Some(TypedBox { unboxed, typ });
        }
    }

    bb.exprs.splice(0..0, additional_expr_assigns);

    next_type_args
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedBox {
    pub unboxed: LocalId,
    pub typ: ValType,
}

// 変数の不変判定
pub fn analyze_locals_immutability(
    locals: &TiVec<LocalId, LocalType>,
    bb: &BasicBlock,
    args: &Vec<LocalId>,
) -> TiVec<LocalId, bool> {
    let mut assign_counts = ti_vec![0; locals.len()];
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
        .map(|count| count <= 1)
        .collect::<TiVec<LocalId, bool>>()
}

/*
コピー伝播を行う
対象はmove, box-unbox, unbox-box

例:

b = move a
c = f b
-->
b = move a
c = f a

b = box<int> a
c = unbox<int> b
d = g c
-->
b = box<int> a
c = unbox<int> a
d = g a

b = unbox<int> a
c = box<int> b
d = g c
-->
b = unbox<int> a
c = box<int> a
d = g a

ここでは、デッドコードの削除は行わない
*/
pub fn copy_propagate(
    locals: &TiVec<LocalId, LocalType>,
    bb: &mut BasicBlock,
    locals_immutability: &TiVec<LocalId, bool>,
) {
    // ローカル変数の置き換え情報
    let mut local_replacements = TiVec::new();
    for local in locals.keys() {
        local_replacements.push(local);
    }

    let mut box_replacements = ti_vec![None; locals.len()];
    let mut unbox_replacements = ti_vec![None; locals.len()];

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
                expr: Box(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
                box_replacements[local] = Some((value, typ));

                if let Some((unboxed, unboxed_typ)) = unbox_replacements[value]
                    && unboxed_typ == typ
                // TODO: unboxed_typ == typは必要？
                {
                    local_replacements[local] = unboxed;
                }
            }
            ExprAssign {
                local: Some(local),
                expr: Unbox(typ, value),
            } if locals_immutability[value] && locals_immutability[local] => {
                unbox_replacements[local] = Some((value, typ));

                if let Some((boxed, boxed_typ)) = box_replacements[value]
                    && boxed_typ == typ
                {
                    local_replacements[local] = boxed;
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
    locals: &TiVec<LocalId, LocalType>,
    bb: &mut BasicBlock,
    _locals_immutability: &TiVec<LocalId, bool>,
    // 別のBBなどで使われているローカル変数
    out_used_locals: &Vec<LocalId>,
) {
    let mut used = ti_vec![false; locals.len()];
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
    // TODO: test_analyze_locals_immutability / test_remove_box以外のテストが微妙なので書き直す
    use super::*;
    use crate::fxbihashmap::FxBiHashMap;
    use typed_index_collections::{TiVec, ti_vec};

    #[test]
    fn test_analyze_locals_immutability() {
        let locals = ti_vec![
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Val(ValType::Int)),
        ];

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
            next: BasicBlockNext::Return(LocalId::from(1)),
        };

        let args = vec![LocalId::from(0), LocalId::from(1)];

        let immutability = analyze_locals_immutability(&locals, &bb, &args);

        assert_eq!(immutability, ti_vec![false, true, false, true]);
    }

    fn create_test_basic_block() -> BasicBlock {
        BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![],
            next: BasicBlockNext::Return(LocalId::from(0)),
        }
    }

    fn create_test_locals() -> TiVec<LocalId, LocalType> {
        ti_vec![
            LocalType::Type(Type::Val(ValType::Int)),  // l0
            LocalType::Type(Type::Boxed),              // l1
            LocalType::Type(Type::Val(ValType::Bool)), // l2
        ]
    }

    #[test]
    fn test_remove_box_with_empty_type_args() {
        let mut locals = create_test_locals();
        let mut bb = create_test_basic_block();
        let type_params = FxBiHashMap::default();
        let type_args = ti_vec![];
        let mut locals_immutability = ti_vec![true; locals.len()];

        let next_type_args = remove_box(
            &mut locals,
            &mut bb,
            &type_params,
            &type_args,
            &mut locals_immutability,
        );

        assert_eq!(next_type_args.len(), 3);
        assert!(next_type_args.iter().all(|x| x.is_none()));
        assert_eq!(bb.exprs.len(), 0);
    }

    #[test]
    fn test_remove_box() {
        let mut locals = ti_vec![
            LocalType::Type(Type::Boxed),
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Val(ValType::Int)),
        ];

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                ExprAssign {
                    local: None,
                    expr: Expr::Cons(LocalId::from(0), LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Unbox(ValType::Int, LocalId::from(0)),
                },
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Unbox(ValType::Int, LocalId::from(0)),
                },
                ExprAssign {
                    local: None,
                    expr: Expr::Add(LocalId::from(1), LocalId::from(2)),
                },
            ],
            next: BasicBlockNext::Return(LocalId::from(1)),
        };

        let type_params = FxBiHashMap::from_iter(vec![(TypeParamId::from(0), LocalId::from(0))]);
        let type_args = ti_vec![Some(ValType::Int)];
        let mut locals_immutability = ti_vec![true, true, true];

        // type argsに対応するlocalsは事前に置き換えて渡すことが想定されている
        locals[LocalId::from(0)] = LocalType::Type(Type::Val(ValType::Int));

        let next_type_args = remove_box(
            &mut locals,
            &mut bb,
            &type_params,
            &type_args,
            &mut locals_immutability,
        );

        assert_eq!(locals, ti_vec![
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Val(ValType::Int)),
            LocalType::Type(Type::Boxed),
        ]);

        assert_eq!(bb.exprs, vec![
            ExprAssign {
                local: Some(LocalId::from(3)),
                expr: Expr::Box(ValType::Int, LocalId::from(0)),
            },
            ExprAssign {
                local: None,
                expr: Expr::Cons(LocalId::from(3), LocalId::from(3)),
            },
            ExprAssign {
                local: Some(LocalId::from(1)),
                expr: Expr::Unbox(ValType::Int, LocalId::from(3)),
            },
            ExprAssign {
                local: Some(LocalId::from(2)),
                expr: Expr::Unbox(ValType::Int, LocalId::from(3)),
            },
            ExprAssign {
                local: None,
                expr: Expr::Add(LocalId::from(1), LocalId::from(2)),
            },
        ]);

        assert_eq!(locals_immutability, ti_vec![true, true, true, true]);
        assert_eq!(next_type_args, ti_vec![
            // TODO: ここがNoneにならないのAPI設計として汚い
            Some(TypedBox {
                unboxed: LocalId::from(2),
                typ: ValType::Int
            }),
            None,
            None,
            Some(TypedBox {
                unboxed: LocalId::from(0),
                typ: ValType::Int
            })
        ]);
    }

    #[test]
    fn test_remove_box_with_unbox_expr() {
        let mut locals = ti_vec![
            LocalType::Type(Type::Boxed),             // l0 - boxed値
            LocalType::Type(Type::Val(ValType::Int)), // l1 - unbox先
        ];

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![ExprAssign {
                local: Some(LocalId::from(1)),
                expr: Expr::Unbox(ValType::Int, LocalId::from(0)),
            }],
            next: BasicBlockNext::Return(LocalId::from(1)),
        };

        let type_params = FxBiHashMap::default();
        let type_args = ti_vec![];
        let mut locals_immutability = ti_vec![true, true];

        let next_type_args = remove_box(
            &mut locals,
            &mut bb,
            &type_params,
            &type_args,
            &mut locals_immutability,
        );

        // unbox式により、next_type_argsにunboxed情報が設定される
        assert_eq!(
            next_type_args[LocalId::from(0)],
            Some(TypedBox {
                unboxed: LocalId::from(1),
                typ: ValType::Int,
            })
        );
    }

    #[test]
    fn test_copy_propagate_move() {
        let locals = ti_vec![
            LocalType::Type(Type::Val(ValType::Int)), // l0
            LocalType::Type(Type::Val(ValType::Int)), // l1
            LocalType::Type(Type::Val(ValType::Int)), // l2
        ];

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                // l1 = move l0
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Move(LocalId::from(0)),
                },
                // l2 = add(l1, l1)
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Add(LocalId::from(1), LocalId::from(1)),
                },
            ],
            next: BasicBlockNext::Return(LocalId::from(2)),
        };

        let locals_immutability = ti_vec![true, true, true];

        copy_propagate(&locals, &mut bb, &locals_immutability);

        // l1の参照がl0に置き換わる
        if let ExprAssign {
            expr: Expr::Add(left, right),
            ..
        } = &bb.exprs[1]
        {
            assert_eq!(*left, LocalId::from(0));
            assert_eq!(*right, LocalId::from(0));
        } else {
            panic!("Expected Add expr with propagated locals");
        }
    }

    #[test]
    fn test_copy_propagate_box_unbox() {
        let locals = ti_vec![
            LocalType::Type(Type::Val(ValType::Int)), // l0
            LocalType::Type(Type::Boxed),             // l1 - box先
            LocalType::Type(Type::Val(ValType::Int)), // l2 - unbox先
            LocalType::Type(Type::Val(ValType::Int)), // l3
        ];

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                // l1 = box<int> l0
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Box(ValType::Int, LocalId::from(0)),
                },
                // l2 = unbox<int> l1
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Unbox(ValType::Int, LocalId::from(1)),
                },
                // l3 = add(l2, l2)
                ExprAssign {
                    local: Some(LocalId::from(3)),
                    expr: Expr::Add(LocalId::from(2), LocalId::from(2)),
                },
            ],
            next: BasicBlockNext::Return(LocalId::from(3)),
        };

        let locals_immutability = ti_vec![true, true, true, true];

        copy_propagate(&locals, &mut bb, &locals_immutability);

        // box-unboxが最適化され、l2の参照がl0に置き換わる
        if let ExprAssign {
            expr: Expr::Add(left, right),
            ..
        } = &bb.exprs[2]
        {
            assert_eq!(*left, LocalId::from(0));
            assert_eq!(*right, LocalId::from(0));
        } else {
            panic!("Expected Add expr with propagated locals");
        }
    }

    #[test]
    fn test_dead_code_elimination() {
        let locals = ti_vec![
            LocalType::Type(Type::Val(ValType::Int)), // l0
            LocalType::Type(Type::Val(ValType::Int)), // l1
            LocalType::Type(Type::Val(ValType::Int)), // l2
        ];

        let mut bb = BasicBlock {
            id: BasicBlockId::from(0),
            exprs: vec![
                // l1 = add(l0, l0) - 使われない
                ExprAssign {
                    local: Some(LocalId::from(1)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
                // l2 = add(l0, l0) - 使われる
                ExprAssign {
                    local: Some(LocalId::from(2)),
                    expr: Expr::Add(LocalId::from(0), LocalId::from(0)),
                },
            ],
            next: BasicBlockNext::Return(LocalId::from(2)),
        };

        let locals_immutability = ti_vec![true, true, true];
        let out_used_locals = vec![];

        dead_code_elimination(&locals, &mut bb, &locals_immutability, &out_used_locals);

        // l1への代入は削除される
        if let ExprAssign {
            local: None,
            expr: Expr::Nop,
        } = &bb.exprs[0]
        {
            // OK
        } else {
            panic!("Expected first expr to be eliminated");
        }

        // l2への代入は残る
        if let ExprAssign {
            local: Some(local),
            expr: Expr::Add(..),
        } = &bb.exprs[1]
        {
            assert_eq!(*local, LocalId::from(2));
        } else {
            panic!("Expected second expr to remain");
        }
    }
}
