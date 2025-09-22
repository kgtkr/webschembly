use crate::ast;
use crate::compiler_error;
use crate::ir;
use crate::ir_generator;
use crate::ir_generator::Jit;
use crate::ir_generator::JitConfig;
use crate::lexer;
use crate::sexpr_parser;

#[derive(Debug)]
pub struct Compiler {
    ast_generator: ast::ASTGenerator,
    global_manager: ir_generator::GlobalManager,
    jit: Option<Jit>,
}

#[derive(Debug, Clone, Copy)]
pub struct FlatConfig {
    pub enable_jit: bool,
    pub enable_jit_optimization: bool,
}

impl From<FlatConfig> for Config {
    fn from(config: FlatConfig) -> Self {
        Self {
            jit: if config.enable_jit {
                Some(JitConfig {
                    enable_optimization: config.enable_jit_optimization,
                })
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub jit: Option<JitConfig>,
}

impl Compiler {
    pub fn new(config: Config) -> Self {
        Self {
            ast_generator: ast::ASTGenerator::new(),
            global_manager: ir_generator::GlobalManager::new(),
            jit: config.jit.map(Jit::new),
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

        if let Some(jit) = &mut self.jit {
            Ok(jit.register_module(&mut self.global_manager, module))
        } else {
            Ok(module)
        }
    }

    pub fn get_global_id(&self, name: &str) -> Option<i32> {
        self.ast_generator.get_global_id(name).map(|id| id.0 as i32)
    }

    pub fn instantiate_func(&mut self, module_id: ir::ModuleId, func_id: ir::FuncId) -> ir::Module {
        self.jit
            .as_mut()
            .expect("JIT is not enabled")
            .instantiate_func(&mut self.global_manager, module_id, func_id)
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: ir::ModuleId,
        func_id: ir::FuncId,
        bb_id: ir::BasicBlockId,
        index: usize,
    ) -> ir::Module {
        self.jit
            .as_mut()
            .expect("JIT is not enabled")
            .instantiate_bb(module_id, func_id, bb_id, index)
    }
}
