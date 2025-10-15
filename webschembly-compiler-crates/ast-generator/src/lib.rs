#![feature(
    trait_alias,
    associated_type_defaults,
    never_type,
    string_deref_patterns,
    box_patterns
)]

mod ast_generator;
mod defined;
mod desugared;
mod parsed;
mod tail_call;
mod used;

pub use ast_generator::*;
pub use defined::*;
pub use desugared::*;
pub use parsed::*;
pub use tail_call::*;
pub use used::*;
