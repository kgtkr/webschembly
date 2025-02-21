use strum::IntoEnumIterator;

use crate::{ast::*, ir};

pub fn generate_stdlib() -> String {
    let mut result = String::new();
    for builtin in Builtin::iter() {
        result.push_str(&generate_builtin(builtin));
    }
    result
}

fn generate_builtin(builtin: Builtin) -> String {
    let builtin_typ = ir::builtin_func_type(builtin);
    debug_assert_eq!(builtin_typ.rets.len(), 1);
    let args = builtin_typ
        .args
        .iter()
        .enumerate()
        .map(|(i, _)| format!("x{}", i))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "(define ({} {}) ({} {}))",
        builtin.name(),
        args,
        builtin.name(),
        args
    )
}
