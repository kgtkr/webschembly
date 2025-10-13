#![feature(
    string_deref_patterns,
    box_patterns,
    trait_alias,
    never_type,
    associated_type_defaults,
    coroutines,
    stmt_expr_attributes,
    iter_from_coroutine,
    if_let_guard,
    impl_trait_in_assoc_type
)]
#![allow(clippy::vec_init_then_push)]
pub mod lexer;
pub mod parser_combinator;
#[macro_use]
pub mod sexpr;
pub mod ast;
pub mod compiler;
pub mod wasm_generator;
#[macro_use]
pub mod error;
pub mod fxbihashmap;
mod has_id;
pub use has_id::HasId;
pub mod ir;
pub mod ir_generator;
pub mod sexpr_parser;
pub mod span;
pub mod stdlib;
pub mod token;
pub mod tokens;
mod vec_map;
pub use vec_map::VecMap;
pub mod ir_processor;
pub mod jit;
pub mod x;
