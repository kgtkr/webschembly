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
    ir_generator: ir_generator::IrGenerator,
    config: Config,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub enable_jit: bool,
}

impl Compiler {
    pub fn new(config: Config) -> Self {
        Self {
            ast_generator: ast::ASTGenerator::new(),
            ir_generator: ir_generator::IrGenerator::new(),
            config,
        }
    }

    pub fn compile_module(
        &mut self,
        input: &str,
        is_stdlib: bool,
    ) -> crate::error::Result<&ir::Module> {
        let tokens = lexer::lex(input)?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| compiler_error!("{}", e))?;
        let ast = self.ast_generator.gen_ast(sexprs)?;
        let module =
            ir_generator::generate_module(&mut self.ir_generator, &ast, ir_generator::Config {
                allow_set_builtin: is_stdlib,
            });
        let module_id = if self.config.enable_jit {
            ir_generator::split_and_register_module(&mut self.ir_generator, module)
        } else {
            self.ir_generator.register_module(module)
        };
        Ok(self.ir_generator.get_module(module_id))
    }

    pub fn compile(&mut self, input: &str, is_stdlib: bool) -> crate::error::Result<Vec<u8>> {
        let module = self.compile_module(input, is_stdlib)?;
        let code = wasm_generator::generate(&module);
        Ok(code)
    }

    pub fn get_global_id(&self, name: &str) -> Option<i32> {
        self.ast_generator.get_global_id(name).map(|id| id.0 as i32)
    }

    pub fn instantiate_module(&self, module_id: ir::ModuleId) -> Vec<u8> {
        let module = self.ir_generator.get_module(module_id);
        wasm_generator::generate(module)
    }
}
