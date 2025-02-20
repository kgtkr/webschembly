mod ast;
mod defined;
mod parsed;
pub use ast::*;
pub use defined::*;
pub use parsed::*;

use crate::sexpr::SExpr;
use anyhow::Result;

pub type Final = Defined;
pub fn parse_and_process(sexprs: Vec<SExpr>) -> Result<AST<Final>> {
    let parsed = AST::<Parsed>::from_sexprs(sexprs)?;
    let defined = AST::<Defined>::from_ast(parsed)?;
    Ok(defined)
}
