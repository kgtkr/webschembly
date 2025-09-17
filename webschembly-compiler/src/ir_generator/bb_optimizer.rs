use typed_index_collections::{TiVec, ti_vec};

use crate::ir::*;

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
*/

pub fn remove_box(
    locals: &mut TiVec<LocalId, LocalType>,
    bb: BasicBlock,
    type_params: &TiVec<TypeParamId, LocalId>,
    type_args: &TiVec<TypeParamId, Option<ValType>>,
) -> BasicBlock {
    let mut expr_assigns = Vec::new();

    // ローカルをBoxed -> Typeに書き換え
    for (type_param_id, &local) in type_params.iter_enumerated() {
        if let Some(typ) = type_args[type_param_id] {
            debug_assert_eq!(locals[local], LocalType::Type(Type::Boxed));
            locals[local] = LocalType::Type(Type::Val(typ));
        }
    }

    let mut type_params_rev = ti_vec![None; locals.len()];
    for (type_param_id, &local) in type_params.iter_enumerated() {
        type_params_rev[local] = Some(type_param_id);
    }
    let type_params_rev = type_params_rev;

    // 型代入されている変数のboxed版を用意(l1_boxedに対応)
    let mut boxed_locals = ti_vec![None; locals.len()];
    for (type_param_id, typ) in type_args.iter_enumerated() {
        if let Some(typ) = typ {
            let boxed_local = locals.push_and_get_key(LocalType::Type(Type::Boxed));
            boxed_locals[type_params[type_param_id]] = Some(boxed_local);

            expr_assigns.push(ExprAssign {
                local: Some(boxed_local),
                expr: Expr::Box(*typ, type_params[type_param_id]),
            });
        }
    }

    let locals = locals;
    let boxed_locals = boxed_locals;

    // 再代入されている変数の特定
    let mut assign_counts = ti_vec![0; locals.len()];
    // TODO: 引数は1度代入されているとみなす
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
    let mut local_replacements = boxed_locals.clone();

    for expr_assign in bb.exprs {
        use Expr::*;
        let new_expr_assign = match expr_assign {
            ExprAssign {
                local: Some(local),
                expr: Unbox(typ, value),
            } if locals_immutability[value]
                && locals_immutability[local]
                && let Some(type_param_id) = type_params_rev[value]
                && let Some(type_arg) = type_args[type_param_id] =>
            {
                debug_assert_eq!(type_arg, typ);
                local_replacements[local] = Some(value);
                ExprAssign {
                    local: None,
                    expr: Expr::Nop,
                }
            }
            mut expr_assign => {
                for (local, _) in expr_assign.local_usages_mut() {
                    if let Some(replacement) = local_replacements[*local] {
                        *local = replacement;
                    }
                }
                expr_assign
            }
        };
        expr_assigns.push(new_expr_assign);
    }

    let mut new_next = bb.next;
    for local in new_next.local_ids_mut() {
        if let Some(replacement) = local_replacements[*local] {
            *local = replacement;
        }
    }

    BasicBlock {
        id: bb.id,
        exprs: expr_assigns,
        next: new_next,
    }
}
