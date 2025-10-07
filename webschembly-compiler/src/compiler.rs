use rustc_hash::FxHashSet;

use crate::ast;
use crate::compiler_error;
use crate::ir;
use crate::ir_generator;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::desugar::desugar;
use crate::ir_processor::jit::{Jit, JitConfig};
use crate::ir_processor::optimizer::remove_unreachable_bb;
use crate::ir_processor::optimizer::remove_unused_local;
use crate::ir_processor::ssa::{debug_assert_ssa, remove_phi};
use crate::ir_processor::ssa_optimizer::ssa_optimize;
use crate::lexer;
use crate::sexpr_parser;

#[derive(Debug)]
pub struct Compiler {
    module_count: usize,
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
            module_count: 0,
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
        // TODO: ここで生成するべきではない
        let module_id = ir::ModuleId::from(self.module_count);
        self.module_count += 1;
        let mut module = ir_generator::generate_module(
            module_id,
            &mut self.global_manager,
            &ast,
            ir_generator::Config {
                allow_set_builtin: is_stdlib,
            },
        );

        preprocess_module(&mut module);
        if let Some(jit) = &mut self.jit {
            let mut stub_module = jit.register_module(&mut self.global_manager, module);
            preprocess_module(&mut stub_module);
            if jit.config().enable_optimization {
                optimize_module(&mut stub_module);
            }
            postprocess(&mut stub_module, &mut self.global_manager);
            Ok(stub_module)
        } else {
            optimize_module(&mut module);
            postprocess(&mut module, &mut self.global_manager);
            Ok(module)
        }
    }

    pub fn instantiate_func(
        &mut self,
        module_id: ir::ModuleId,
        func_id: ir::FuncId,
        func_index: usize,
    ) -> ir::Module {
        let jit = self.jit.as_mut().expect("JIT is not enabled");
        let mut module =
            jit.instantiate_func(&mut self.global_manager, module_id, func_id, func_index);
        preprocess_module(&mut module);
        if jit.config().enable_optimization {
            optimize_module(&mut module);
        }
        postprocess(&mut module, &mut self.global_manager);
        module
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: ir::ModuleId,
        func_id: ir::FuncId,
        func_index: usize,
        bb_id: ir::BasicBlockId,
        index: usize,
    ) -> ir::Module {
        let jit = self.jit.as_mut().expect("JIT is not enabled");
        let mut module = jit.instantiate_bb(
            module_id,
            func_id,
            func_index,
            bb_id,
            index,
            &mut self.global_manager,
        );
        preprocess_module(&mut module);
        if jit.config().enable_optimization {
            optimize_module(&mut module);
        }
        postprocess(&mut module, &mut self.global_manager);
        module
    }
}

fn preprocess_module(module: &mut ir::Module) {
    for func in module.funcs.iter_mut() {
        debug_assert_ssa(func);
        remove_unreachable_bb(func);
    }
}

fn optimize_module(module: &mut ir::Module) {
    for func in module.funcs.iter_mut() {
        ssa_optimize(func);
    }
}

fn postprocess(module: &mut ir::Module, global_manager: &mut GlobalManager) {
    for func in module.funcs.iter_mut() {
        debug_assert_ssa(func);

        desugar(func);

        // TODO: クリティカルエッジの分割
        remove_phi(func);
        // TODO: レジスタ割り付け

        remove_unused_local(func);
    }

    // モジュールごとにグローバルを真面目に管理するのは大変なのでここで計算
    let mut global_ids = FxHashSet::default();
    for func in module.funcs.iter() {
        for bbs in func.bbs.values() {
            for expr_assign in bbs.exprs.iter() {
                if let ir::Expr::GlobalGet(global_id) | ir::Expr::GlobalSet(global_id, _) =
                    expr_assign.expr
                {
                    global_ids.insert(global_id);
                }
            }
        }
    }
    module.globals =
        global_manager.calc_module_globals(&global_ids.iter().copied().collect::<Vec<_>>());
}
