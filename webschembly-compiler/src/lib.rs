pub mod lexer;
pub mod sexpr;
pub mod token;

#[no_mangle]
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}
