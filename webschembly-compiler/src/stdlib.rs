use strum::IntoEnumIterator;

use crate::ast::Builtin;
use crate::ir_generator::BuiltinConversionRule;

pub fn generate_stdlib() -> String {
    let mut result = String::new();
    for builtin in Builtin::iter() {
        result.push_str(&generate_builtin(builtin));
    }
    result.push_str(include_str!("stdlib.scm"));
    result
}

fn generate_builtin(builtin: Builtin) -> String {
    let rule = BuiltinConversionRule::from_builtin(builtin);
    let args = (0..rule.args_count())
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
