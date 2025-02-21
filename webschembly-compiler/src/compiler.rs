use crate::ast;
use crate::codegen;
use crate::ir;
use crate::lexer;
use crate::sexpr_parser;
use crate::stdlib;

pub struct Compiler {
    ast_gen: ast::ASTGenerator,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            ast_gen: ast::ASTGenerator::new(),
        }
    }

    pub fn compile(&mut self, input: &str) -> anyhow::Result<Vec<u8>> {
        let stdlib = stdlib::generate_stdlib();

        // モジュール化の仕組みを整える
        let input = format!("{}\n{}", stdlib, input);

        let tokens = lexer::lex(&input).map_err(|e| anyhow::anyhow!("{}", e))?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| anyhow::anyhow!("{}", e))?;
        let ast = self.ast_gen.gen_ast(sexprs)?;
        let ir = ir::Ir::from_ast(&ast)?;
        let code = codegen::ModuleGenerator::new().gen(&ir);
        Ok(code.finish())
    }
}
