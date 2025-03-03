use crate::ast;
use crate::codegen;
use crate::compiler_error;
use crate::ir;
use crate::lexer;
use crate::sexpr_parser;

pub struct Compiler {
    ast_gen: ast::ASTGenerator,
    codegen: codegen::Codegen,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            ast_gen: ast::ASTGenerator::new(),
            codegen: codegen::Codegen::new(),
        }
    }

    pub fn compile(&mut self, input: &str, is_stdlib: bool) -> crate::error::Result<Vec<u8>> {
        let tokens = lexer::lex(input)?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| compiler_error!("{}", e))?;
        let ast = self.ast_gen.gen_ast(sexprs)?;
        let ir = ir::Ir::from_ast(&ast, ir::Config {
            allow_set_builtin: is_stdlib,
        });
        let code = self.codegen.generate(&ir);
        Ok(code)
    }
}
