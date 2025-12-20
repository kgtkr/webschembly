use strum::IntoEnumIterator;

use crate::ir_generator::BuiltinConversionRule;
use webschembly_compiler_ast::Builtin;

pub fn generate_stdlib() -> String {
    let mut result = String::new();
    result.push_str(include_str!("stdlib.scm"));
    for builtin in Builtin::iter() {
        result.push_str(&generate_builtin(builtin));
    }
    result
}

fn generate_builtin(builtin: Builtin) -> String {
    let rule = BuiltinConversionRule::from_builtin(builtin)[0]; // TODO: オーバーロード対応
    let args = (0..rule.arg_count())
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
