use crate::{
    defined::Defined,
    desugared::Desugared,
    parsed::Parsed,
    tail_call::TailCall,
    used::{GlobalVarId, Used, VarIdGen},
};
use webschembly_compiler_ast::Ast;
use webschembly_compiler_error::Result;
use webschembly_compiler_sexpr::LSExpr;

pub type Final = Used<TailCall<Defined<Desugared<Parsed>>>>;

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

    pub fn gen_ast(&mut self, sexprs: Vec<LSExpr>) -> Result<Ast<Final>> {
        let parsed = Parsed::from_sexprs(sexprs)?;
        let desugared = Desugared::new().from_ast(parsed);
        let defined = Defined::from_ast(desugared)?;
        let tail_call = TailCall::from_ast(defined);
        let used = Used::from_ast(tail_call, &mut self.var_id_gen)?;
        Ok(used)
    }

    pub fn get_global_id(&self, name: &str) -> Option<GlobalVarId> {
        self.var_id_gen.get_global_id(name)
    }
}
