use typed_index_collections::TiVec;

use crate::ir::*;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::bb_optimizer;
mod jit_config;
mod jit_module;
pub use jit_config::JitConfig;
use jit_module::JitModule;
mod jit_ctx;
use jit_ctx::JitCtx;

#[derive(Debug)]
pub struct Jit {
    jit_module: TiVec<ModuleId, JitModule>,
    ctx: JitCtx,
}

impl Jit {
    pub fn new(config: JitConfig) -> Self {
        Self {
            jit_module: TiVec::new(),
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
        let module_id = self.jit_module.next_key();
        self.jit_module
            .push(JitModule::new(global_manager, module_id, module));
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
            module_id,
            func_id,
            func_index,
            bb_id,
            index,
            global_manager,
            &mut self.ctx,
        )
    }
}
