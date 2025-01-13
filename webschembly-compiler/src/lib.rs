pub mod lexer;
pub mod parser_combinator;
#[macro_use]
pub mod sexpr;
pub mod sexpr_parser;
pub mod token;

#[no_mangle]
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}
