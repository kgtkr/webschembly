use crate::ast;
use crate::compiler_error;
use crate::ir;
use crate::ir_generator;
use crate::lexer;
use crate::sexpr_parser;
use crate::wasm_generator;

#[derive(Debug)]
pub struct Compiler {
    ast_generator: ast::ASTGenerator,
    wasm_generator: wasm_generator::WasmGenerator,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            ast_generator: ast::ASTGenerator::new(),
            wasm_generator: wasm_generator::WasmGenerator::new(),
        }
    }

    pub fn compile_ir(
        &mut self,
        input: &str,
        is_stdlib: bool,
    ) -> crate::error::Result<(ir::Module, ir::Meta)> {
        let tokens = lexer::lex(input)?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| compiler_error!("{}", e))?;
        let ast = self.ast_generator.gen_ast(sexprs)?;
        Ok(ir_generator::generate_ir(
            &ast,
            ir_generator::Config {
                allow_set_builtin: is_stdlib,
            },
            self.ast_generator.local_metas().clone(),
            self.ast_generator.global_metas().clone(),
        ))
    }

    pub fn compile(&mut self, input: &str, is_stdlib: bool) -> crate::error::Result<Vec<u8>> {
        let (ir, _) = self.compile_ir(input, is_stdlib)?;
        let code = self.wasm_generator.generate(&ir);
        Ok(code)
    }

    pub fn get_global_id(&self, name: &str) -> Option<i32> {
        self.ast_generator.get_global_id(name).map(|id| id.0 as i32)
    }
}
