use strum::IntoEnumIterator;

use crate::ast::Builtin;

pub fn generate_stdlib() -> String {
    let mut result = String::new();
    for builtin in Builtin::iter() {
        result.push_str(&generate_builtin(builtin));
    }
    result.push_str(include_str!("stdlib.scm"));
    result
}

fn generate_builtin(builtin: Builtin) -> String {
    let builtin_typ = builtin.typ();
    let args = (0..builtin_typ.args_count)
        .map(|i| format!("x{}", i))
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
