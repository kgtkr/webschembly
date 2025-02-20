mod ast;
mod defined;
mod parsed;
mod used;
pub use ast::*;
pub use defined::*;
pub use parsed::*;
pub use used::*;

use crate::sexpr::SExpr;
use anyhow::Result;

pub type Final = Used;
pub fn parse_and_process(sexprs: Vec<SExpr>) -> Result<AST<Final>> {
    let parsed = AST::<Parsed>::from_sexprs(sexprs)?;
    let defined = AST::<Defined>::from_ast(parsed)?;
    let used = AST::<Used>::from_ast(defined);
    // TODO: 末尾位置解析
    Ok(used)
}
