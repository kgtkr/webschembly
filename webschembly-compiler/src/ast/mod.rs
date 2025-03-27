mod astx;
mod defined;
mod desugared;
mod parsed;
mod used;
pub use astx::*;
pub use defined::*;
pub use desugared::*;
pub use parsed::*;
pub use used::*;

use crate::error::Result;
use crate::sexpr::SExpr;

pub type Final = Used;

#[derive(Debug)]
pub struct ASTGenerator {
    var_id_gen: VarIdGen,
}

impl Default for ASTGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ASTGenerator {
    pub fn new() -> Self {
        Self {
            var_id_gen: VarIdGen::new(),
        }
    }

    pub fn gen_ast(&mut self, sexprs: Vec<SExpr>) -> Result<Ast<Final>> {
        let parsed = Ast::<Parsed>::from_sexprs(sexprs)?;
        let desugared = Ast::<Desugared>::from_ast(parsed);
        let defined = Ast::<Defined>::from_ast(desugared)?;
        let used = Ast::<Used>::from_ast(defined, &mut self.var_id_gen);
        // TODO: 末尾位置解析
        Ok(used)
    }
}
