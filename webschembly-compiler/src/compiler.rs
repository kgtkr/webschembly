use crate::ast;
use crate::compiler_error;
use crate::ir;
use crate::ir_generator;
use crate::ir_generator::Jit;
use crate::lexer;
use crate::sexpr_parser;

#[derive(Debug)]
pub struct Compiler {
    ast_generator: ast::ASTGenerator,
    global_manager: ir_generator::GlobalManager,
    jit: Jit,
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
            global_manager: ir_generator::GlobalManager::new(),
            jit: Jit::new(),
            config,
        }
    }

    pub fn compile_module(
        &mut self,
        input: &str,
        is_stdlib: bool,
    ) -> crate::error::Result<ir::Module> {
        let tokens = lexer::lex(input)?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| compiler_error!("{}", e))?;
        let ast = self.ast_generator.gen_ast(sexprs)?;
        let module =
            ir_generator::generate_module(&mut self.global_manager, &ast, ir_generator::Config {
                allow_set_builtin: is_stdlib,
            });

        if self.config.enable_jit {
            Ok(self.jit.register_module(&mut self.global_manager, module))
        } else {
            Ok(module)
        }
    }

    pub fn get_global_id(&self, name: &str) -> Option<i32> {
        self.ast_generator.get_global_id(name).map(|id| id.0 as i32)
    }

    pub fn instantiate_func(&self, module_id: ir::ModuleId, func_id: ir::FuncId) -> ir::Module {
        if !self.config.enable_jit {
            panic!("JIT is not enabled");
        }

        self.jit.instantiate_func(module_id, func_id)
    }
}
