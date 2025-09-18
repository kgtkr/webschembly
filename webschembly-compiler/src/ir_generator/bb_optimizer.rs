use typed_index_collections::{TiVec, ti_vec};

use crate::{fxbihashmap::FxBiHashMap, ir::*};

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
_ = cons(l1_boxed, l1_boxed) // unbox以外でl1を参照している式はl1_boxedに置き換える
_ = nop // unboxでl1を参照している場合、nopに置き換え、左辺を記憶
_ = nop // 同様
_ = add(l1, l1) // 上で記憶した左辺が出てきたらl1に置き換え
```

次のBBに引き継ぐ情報:
- boxedな値→unboxedな値の対応とその型(どちらも再代入されない場合のみ)
*/

pub fn remove_box(
    locals: &mut TiVec<LocalId, LocalType>,
    bb: BasicBlock,
    type_params: &FxBiHashMap<TypeParamId, LocalId>,
    type_args: &TiVec<TypeParamId, Option<ValType>>,
    args: &Vec<LocalId>,
) -> (BasicBlock, TiVec<LocalId, Option<NextTypeArg>>) {
    let mut expr_assigns = Vec::new();

    // 型代入されている変数のboxed版を用意(l1_boxedに対応)
    let mut boxed_locals = ti_vec![None; locals.len()];
    for (type_param_id, typ) in type_args.iter_enumerated() {
        if let Some(typ) = typ {
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));
            boxed_locals[*type_params.get_by_left(&type_param_id).unwrap()] = Some(boxed_local);

            expr_assigns.push(ExprAssign {
                local: Some(boxed_local),
                expr: Expr::Box(*typ, *type_params.get_by_left(&type_param_id).unwrap()),
            });
        }
    }

    let boxed_locals = boxed_locals;

    // 再代入されている変数の特定
    let mut assign_counts = ti_vec![0; locals.len()];
    for &arg in args {
        assign_counts[arg] += 1;
    }
    for expr_assign in &bb.exprs {
        if let Some(local) = expr_assign.local {
            assign_counts[local] += 1;
        }
    }

    let locals_immutability = assign_counts
        .into_iter()
        .map(|count| count <= 1)
        .collect::<TiVec<LocalId, bool>>();

    // ローカル変数の置き換え情報
    let mut local_replacements = TiVec::new();
    for (from, to) in boxed_locals.iter_enumerated() {
        local_replacements.push(to.unwrap_or(from));
    }

    // 次のBBに引き継ぐ型情報
    let mut next_type_args = ti_vec![None; locals.len()];

    for mut expr_assign in bb.exprs {
        use Expr::*;

        expr_assign = match expr_assign {
            ExprAssign {
                local,
                expr: Unbox(typ, value),
            } => {
                if locals_immutability[value]
                    && let Some(local) = local
                    && locals_immutability[local]
                    && let Some(&type_param_id) = type_params.get_by_right(&value)
                    && let Some(type_arg) = type_args[type_param_id]
                {
                    debug_assert_eq!(type_arg, typ);
                    local_replacements[local] = value;
                    ExprAssign {
                        local: None,
                        expr: Expr::Nop,
                    }
                } else {
                    if locals_immutability[value]
                        && let Some(local) = local
                        && locals_immutability[local]
                    {
                        next_type_args[value] = Some(NextTypeArg {
                            unboxed: local,
                            typ,
                        });
                    }
                    expr_assign
                }
            }
            expr_assign => expr_assign,
        };
        for (local, _) in expr_assign.local_usages_mut() {
            *local = local_replacements[*local];
        }
        expr_assigns.push(expr_assign);
    }

    let mut new_next = bb.next;
    for local in new_next.local_ids_mut() {
        *local = local_replacements[*local]
    }

    for (unboxed, &boxed) in boxed_locals.iter_enumerated() {
        if let Some(boxed) = boxed
            && locals_immutability[unboxed]
            && locals_immutability[boxed]
        {
            let LocalType::Type(Type::Val(typ)) = locals[unboxed] else {
                unreachable!()
            };
            next_type_args[boxed] = Some(NextTypeArg { unboxed, typ });
        }
    }

    (
        BasicBlock {
            id: bb.id,
            exprs: expr_assigns,
            next: new_next,
        },
        next_type_args,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct NextTypeArg {
    pub unboxed: LocalId,
    pub typ: ValType,
}

pub fn remove_move(
    locals: &TiVec<LocalId, LocalType>,
    mut bb: BasicBlock,
    args: &Vec<LocalId>,
) -> BasicBlock {
    // 再代入されている変数の特定
    let mut assign_counts = ti_vec![0; locals.len()];
    for &arg in args {
        assign_counts[arg] += 1;
    }
    for expr_assign in &bb.exprs {
        if let Some(local) = expr_assign.local {
            assign_counts[local] += 1;
        }
    }

    let locals_immutability = assign_counts
        .into_iter()
        .map(|count| count <= 1)
        .collect::<TiVec<LocalId, bool>>();

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

    bb
}
