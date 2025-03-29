use strum::IntoEnumIterator;

use crate::ir::Builtin;

pub fn generate_stdlib() -> String {
    let mut result = String::new();
    for builtin in Builtin::iter() {
        result.push_str(&generate_builtin(builtin));
    }
    result.push_str(include_str!("stdlib.scm"));
    result
}

fn generate_builtin(builtin: Builtin) -> String {
    let builtin_typ = builtin.func_type();
    debug_assert_eq!(builtin_typ.rets.len(), 1);
    let args = builtin_typ
        .args
        .iter()
        .enumerate()
        .map(|(i, _)| format!("x{}", i))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "(define ({} {}) ({} {}))\n",
        builtin.name(),
        args,
        builtin.name(),
        args
    )
}
