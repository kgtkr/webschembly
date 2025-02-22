use crate::ast;
use crate::codegen;
use crate::ir;
use crate::lexer;
use crate::sexpr_parser;

pub struct Compiler {
    ast_gen: ast::ASTGenerator,
    codegen: codegen::Codegen,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            ast_gen: ast::ASTGenerator::new(),
            codegen: codegen::Codegen::new(),
        }
    }

    pub fn compile(&mut self, input: &str, is_stdlib: bool) -> anyhow::Result<Vec<u8>> {
        let tokens = lexer::lex(&input).map_err(|e| anyhow::anyhow!("{}", e))?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| anyhow::anyhow!("{}", e))?;
        let ast = self.ast_gen.gen_ast(sexprs)?;
        let ir = ir::Ir::from_ast(
            &ast,
            ir::Config {
                allow_set_builtin: is_stdlib,
            },
        );
        let code = self.codegen.gen(&ir)?;
        Ok(code)
    }
}
