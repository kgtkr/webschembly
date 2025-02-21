#![feature(string_deref_patterns, box_patterns, trait_alias, never_type)]
pub mod lexer;
pub mod parser_combinator;
#[macro_use]
pub mod sexpr;
pub mod ast;
pub mod codegen;
pub mod compiler;
pub mod ir;
pub mod sexpr_parser;
pub mod stdlib;
pub mod token;
pub mod x;

pub fn compile(input: &str) -> anyhow::Result<Vec<u8>> {
    let mut compiler = compiler::Compiler::new();
    compiler.compile(input)
}
