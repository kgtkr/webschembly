mod astx;
mod builtin;
mod defined;
mod desugared;
mod parsed;
mod tail_call;
mod used;
pub use astx::*;
pub use builtin::*;
pub use defined::*;
pub use desugared::*;
pub use parsed::*;
pub use tail_call::*;
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
        let tail_call = Ast::<TailCall>::from_ast(defined);
        let used = Ast::<Used>::from_ast(tail_call, &mut self.var_id_gen);
        Ok(used)
    }

    pub fn get_global_id(&self, name: &str) -> Option<GlobalVarId> {
        self.var_id_gen.get_global_id(name)
    }
}
