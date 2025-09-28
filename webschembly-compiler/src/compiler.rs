use crate::VecMap;
use crate::ast;
use crate::cfg::preprocess_cfg;
use crate::compiler_error;
use crate::ir;
use crate::ir_generator;
use crate::ir_generator::Jit;
use crate::ir_generator::JitConfig;
use crate::ir_generator::remove_phi;
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

    pub fn get_global_id(&self, name: &str) -> Option<i32> {
        let global_var_id = self.ast_generator.get_global_id(name)?;
        let global_id = self.global_manager.get_global_id(global_var_id)?;
        Some(usize::from(global_id) as i32)
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
        let mut module =
            ir_generator::generate_module(&mut self.global_manager, &ast, ir_generator::Config {
                allow_set_builtin: is_stdlib,
            });
        preprocess_module(&mut module);

        if let Some(jit) = &mut self.jit {
            Ok(jit.register_module(&mut self.global_manager, module))
        } else {
            Ok(module)
        }
    }

    pub fn instantiate_func(&mut self, module_id: ir::ModuleId, func_id: ir::FuncId) -> ir::Module {
        let mut module = self
            .jit
            .as_mut()
            .expect("JIT is not enabled")
            .instantiate_func(&mut self.global_manager, module_id, func_id);
        preprocess_module(&mut module);
        module
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: ir::ModuleId,
        func_id: ir::FuncId,
        bb_id: ir::BasicBlockId,
        index: usize,
    ) -> ir::Module {
        let mut module = self
            .jit
            .as_mut()
            .expect("JIT is not enabled")
            .instantiate_bb(module_id, func_id, bb_id, index);
        preprocess_module(&mut module);
        module
    }
}

fn preprocess_module(module: &mut ir::Module) {
    for func in module.funcs.iter_mut() {
        if cfg!(debug_assertions) {
            assert_ssa(func);
        }
        preprocess_cfg(&mut func.bbs, func.bb_entry);
        if cfg!(debug_assertions) {
            assert_ssa(func);
        }

        // TODO: クリティカルエッジの分割
        remove_phi(func);
        // TODO: レジスタ割り当て

        // 未使用のローカルを削除
        let mut local_used = VecMap::new();
        for local_id in func.locals.keys() {
            local_used.insert(local_id, false);
        }
        for &local_id in func.args.iter() {
            local_used[local_id] = true;
        }
        for bb in func.bbs.values() {
            for (&local, _) in bb.local_usages() {
                local_used[local] = true;
            }
        }
        func.locals.retain(|local_id, _| local_used[local_id]);
    }
}

fn assert_ssa(func: &ir::Func) {
    let mut assigned = VecMap::new();
    for local_id in func.locals.keys() {
        assigned.insert(local_id, false);
    }
    for &local_id in func.args.iter() {
        assigned[local_id] = true;
    }
    for bb in func.bbs.values() {
        for expr in bb.exprs.iter() {
            if let Some(local_id) = expr.local {
                if assigned[local_id] {
                    panic!("local {:?} is assigned more than once", local_id);
                }
                assigned[local_id] = true;
            }
        }
    }
}
