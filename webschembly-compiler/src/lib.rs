pub mod token;
pub mod lexer;

#[no_mangle]
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}
