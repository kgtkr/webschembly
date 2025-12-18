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
    impl_trait_in_assoc_type,
    coroutine_trait
)]
#![allow(clippy::vec_init_then_push, clippy::too_many_arguments)]
pub mod compiler;
pub mod fxbihashmap;
pub mod ir_generator;
pub mod ir_processor;
pub mod jit;
pub mod lexer;
pub mod parser_combinator;
pub mod sexpr_parser;
pub mod stdlib;
pub mod token;
pub mod tokens;
pub mod wasm_generator;
