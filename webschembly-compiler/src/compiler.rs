use rustc_hash::FxHashSet;

use crate::ir_generator;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::desugar::desugar;
use crate::ir_processor::optimizer::remove_unreachable_bb;
use crate::ir_processor::optimizer::remove_unused_local;
use crate::ir_processor::ssa::{debug_assert_ssa, remove_phi};
use crate::ir_processor::ssa_optimizer::ModuleInliner;
use crate::ir_processor::ssa_optimizer::SsaOptimizerConfig;
use crate::ir_processor::ssa_optimizer::inlining;
use crate::ir_processor::ssa_optimizer::ssa_optimize;
use crate::jit::{Jit, JitConfig};
use crate::lexer;
use crate::sexpr_parser;
use webschembly_compiler_ast_generator::ASTGenerator;
use webschembly_compiler_error::compiler_error;
use webschembly_compiler_ir as ir;

#[derive(Debug)]
pub struct Compiler {
    module_count: usize,
    ast_generator: ASTGenerator,
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
            ast_generator: ASTGenerator::new(),
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
    ) -> webschembly_compiler_error::Result<ir::Module> {
        let tokens = lexer::lex(input)?;
        let sexprs =
            sexpr_parser::parse(tokens.as_slice()).map_err(|e| compiler_error!("{}", e))?;
        let ast = self.ast_generator.gen_ast(sexprs)?;
        // TODO: ここで生成するべきではない
        let module_id = ir::JitModuleId::from(self.module_count);
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
            optimize_module(
                &mut module,
                SsaOptimizerConfig {
                    enable_cse: false,
                    ..Default::default()
                },
            );
            let mut stub_module = jit.register_module(&mut self.global_manager, module);
            preprocess_module(&mut stub_module);
            if jit.config().enable_optimization {
                optimize_module(
                    &mut stub_module,
                    SsaOptimizerConfig {
                        enable_inlining: false,
                        ..Default::default()
                    },
                );
            }
            postprocess(&mut stub_module, &mut self.global_manager);
            Ok(stub_module)
        } else {
            optimize_module(&mut module, Default::default());
            postprocess(&mut module, &mut self.global_manager);

            Ok(module)
        }
    }

    pub fn instantiate_func(
        &mut self,
        module_id: usize,
        func_id: usize,
        func_index: usize,
    ) -> ir::Module {
        let module_id = ir::JitModuleId::from(module_id);
        let func_id = ir::FuncId::from(func_id);
        let jit = self.jit.as_mut().expect("JIT is not enabled");
        let mut module =
            jit.instantiate_func(&mut self.global_manager, module_id, func_id, func_index);

        preprocess_module(&mut module);
        if jit.config().enable_optimization {
            optimize_module(
                &mut module,
                SsaOptimizerConfig {
                    enable_inlining: false,
                    ..Default::default()
                },
            );
        }
        postprocess(&mut module, &mut self.global_manager);
        module
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: usize,
        func_id: usize,
        func_index: usize,
        bb_id: usize,
        index: usize,
    ) -> ir::Module {
        let module_id = ir::JitModuleId::from(module_id);
        let func_id = ir::FuncId::from(func_id);
        let bb_id = ir::BasicBlockId::from(bb_id);
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
            optimize_module(
                &mut module,
                SsaOptimizerConfig {
                    enable_inlining: false,
                    ..Default::default()
                },
            );
        }
        postprocess(&mut module, &mut self.global_manager);
        module
    }

    pub fn increment_branch_counter(
        &mut self,
        module_id: usize,
        func_id: usize,
        func_index: usize,
        bb_id: usize,
        kind: usize, // 0: Then, 1: Else
        source_bb_id: usize,
        source_index: usize,
    ) -> Option<ir::Module> {
        let module_id = ir::JitModuleId::from(module_id);
        let func_id = ir::FuncId::from(func_id);
        let bb_id = ir::BasicBlockId::from(bb_id);
        let jit = self.jit.as_mut().expect("JIT is not enabled");
        let kind = match kind {
            0 => ir::BranchKind::Then,
            1 => ir::BranchKind::Else,
            _ => panic!("Invalid branch kind"),
        };
        jit.increment_branch_counter(
            &mut self.global_manager,
            module_id,
            func_id,
            func_index,
            bb_id,
            kind,
            ir::BasicBlockId::from(source_bb_id),
            source_index,
        )
        .map(|mut module| {
            preprocess_module(&mut module);
            if jit.config().enable_optimization {
                optimize_module(
                    &mut module,
                    SsaOptimizerConfig {
                        enable_inlining: false,
                        ..Default::default()
                    },
                );
            }
            postprocess(&mut module, &mut self.global_manager);
            module
        })
    }
}

fn preprocess_module(module: &mut ir::Module) {
    for func in module.funcs.values_mut() {
        debug_assert_ssa(func);
        remove_unreachable_bb(func);
    }
}

fn optimize_module(module: &mut ir::Module, config: SsaOptimizerConfig) {
    let mut module_inliner = ModuleInliner::new(module);
    let n = 5;
    for i in 0..n {
        if config.enable_inlining {
            // inliningはInstrKind::Closureのfunc_idに依存しているので、JIT後のモジュールには使えない
            // inlining(module, &mut module_inliner, i == n - 1);
        }
        for func in module.funcs.values_mut() {
            ssa_optimize(func, config);
        }
    }
}

fn postprocess(module: &mut ir::Module, global_manager: &mut GlobalManager) {
    for func in module.funcs.values_mut() {
        debug_assert_ssa(func);

        desugar(func);

        // TODO: クリティカルエッジの分割
        remove_phi(func);
        // TODO: レジスタ割り付け

        remove_unused_local(func);
    }

    // モジュールごとにグローバルを真面目に管理するのは大変なのでここで計算
    let mut global_ids = FxHashSet::default();
    for func in module.funcs.values() {
        for bbs in func.bbs.values() {
            for instr in bbs.instrs.iter() {
                if let ir::InstrKind::GlobalGet(global_id)
                | ir::InstrKind::GlobalSet(global_id, _) = instr.kind
                {
                    global_ids.insert(global_id);
                }
            }
        }
    }
    module.globals =
        global_manager.calc_module_globals(&global_ids.iter().copied().collect::<Vec<_>>());
}
