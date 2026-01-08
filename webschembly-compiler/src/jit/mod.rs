use crate::ir_generator::GlobalManager;
use vec_map::VecMap;

use bb_index_manager::BBIndex;
use closure_global_layout::ClosureIndex;
use env_index_manager::EnvIndex;
mod jit_config;
mod jit_module;
pub use jit_config::JitConfig;
use jit_module::JitModule;
mod jit_ctx;
use jit_ctx::JitCtx;
use webschembly_compiler_ir::*;
pub mod bb_index_manager;
pub mod closure_global_layout;
pub mod env_index_manager;
pub mod index_flag;
mod jit_func;

#[derive(Debug)]
pub struct Jit {
    jit_module: VecMap<JitModuleId, JitModule>,
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
        module: Module,
    ) -> Module {
        let module_id = self
            .jit_module
            .push_with(|id| JitModule::new(global_manager, id, module, &mut self.ctx));
        self.jit_module[module_id].generate_stub_module(global_manager, &mut self.ctx)
    }

    pub fn instantiate_func(
        &mut self,
        global_manager: &mut GlobalManager,
        module_id: JitModuleId,
        func_id: FuncId,
        env_index: EnvIndex,
        func_index: ClosureIndex,
    ) -> Module {
        self.jit_module[module_id].instantiate_func(
            global_manager,
            func_id,
            env_index,
            func_index,
            &mut self.ctx,
        )
    }

    pub fn instantiate_bb(
        &mut self,
        module_id: JitModuleId,
        func_id: FuncId,
        env_index: EnvIndex,
        func_index: ClosureIndex,
        bb_id: BasicBlockId,
        index: BBIndex,
        global_manager: &mut GlobalManager,
    ) -> Module {
        self.jit_module[module_id].instantiate_bb(
            func_id,
            env_index,
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
        module_id: JitModuleId,
        func_id: FuncId,
        env_index: EnvIndex,
        func_index: ClosureIndex,
        bb_id: BasicBlockId,
        kind: BranchKind,
        source_bb_id: BasicBlockId,
        source_index: BBIndex,
    ) -> Option<Module> {
        self.jit_module[module_id].increment_branch_counter(
            global_manager,
            &mut self.ctx,
            func_id,
            env_index,
            func_index,
            bb_id,
            kind,
            source_bb_id,
            source_index,
        )
    }
}
