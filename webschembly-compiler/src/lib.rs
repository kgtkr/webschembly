#![feature(string_deref_patterns, box_patterns)]
pub mod lexer;
pub mod parser_combinator;
#[macro_use]
pub mod sexpr;
pub mod ast;
pub mod codegen;
pub mod ir;
pub mod sexpr_parser;
pub mod token;

#[no_mangle]
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}
