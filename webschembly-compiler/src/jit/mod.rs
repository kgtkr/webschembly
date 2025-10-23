use crate::{
    ir_generator::GlobalManager,
    ir_processor::ssa_optimizer::{SsaOptimizerConfig, inlining, ssa_optimize},
};
use vec_map::VecMap;
mod jit_config;
mod jit_module;
pub use jit_config::JitConfig;
use jit_module::JitModule;
mod jit_ctx;
use jit_ctx::JitCtx;
use webschembly_compiler_ir::*;
mod global_layout;
mod jit_func;

#[derive(Debug)]
pub struct Jit {
    jit_module: VecMap<ModuleId, JitModule>,
    ctx: JitCtx,
}

impl Jit {
    pub fn new(config: JitConfig) -> Self {
        Self {
            jit_module: VecMap::new(),
            ctx: JitCtx::new(config),
        }
    }

    pub fn config(&self) -> JitConfig {
        self.ctx.config()
    }

    pub fn register_module(
        &mut self,
        global_manager: &mut GlobalManager,
        mut module: Module,
    ) -> Module {
        for func in module.funcs.values_mut() {
            ssa_optimize(
                func,
                SsaOptimizerConfig {
                    enable_cse: false,
                    ..Default::default()
                },
            );
        }
        inlining(&mut module);
        let module_id = self
            .jit_module
            .push_with(|id| JitModule::new(global_manager, id, module));
        self.jit_module[module_id].generate_stub_module(global_manager, &mut self.ctx)
    }

    pub fn instantiate_func(
        &mut self,
        global_manager: &mut GlobalManager,
        module_id: ModuleId,
        func_id: FuncId,
        func_index: usize,
    ) -> Module {
        self.jit_module[module_id].instantiate_func(
            global_manager,
            func_id,
            func_index,
            &mut self.ctx,
        )
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: ModuleId,
        func_id: FuncId,
        func_index: usize,
        bb_id: BasicBlockId,
        index: usize,
        global_manager: &mut GlobalManager,
    ) -> Module {
        self.jit_module[module_id].instantiate_bb(
            func_id,
            func_index,
            bb_id,
            index,
            global_manager,
            &mut self.ctx,
        )
    }

    pub fn increment_branch_counter(
        &mut self,
        global_manager: &mut GlobalManager,
        module_id: ModuleId,
        func_id: FuncId,
        func_index: usize,
        bb_id: BasicBlockId,
        kind: BranchKind,
        source_bb_id: BasicBlockId,
        source_index: usize,
    ) -> Option<Module> {
        self.jit_module[module_id].increment_branch_counter(
            global_manager,
            &mut self.ctx,
            func_id,
            func_index,
            bb_id,
            kind,
            source_bb_id,
            source_index,
        )
    }
}
