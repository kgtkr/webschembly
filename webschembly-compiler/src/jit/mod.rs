use rustc_hash::FxHashMap;
use typed_index_collections::TiVec;

use crate::ir::*;
use crate::ir_generator::GlobalManager;
use crate::ir_processor::bb_optimizer;
mod jit_config;
mod jit_module;
pub use jit_config::JitConfig;
use jit_module::{ClosureGlobalLayout, JitModule};

#[derive(Debug)]
pub struct Jit {
    config: JitConfig,
    jit_module: TiVec<ModuleId, JitModule>,
    closure_global_layout: ClosureGlobalLayout,
    // 0..GLOBAL_LAYOUT_MAX_SIZEまでのindexに対応する関数のスタブが入ったMutFuncRef
    // func_indexがインスタンス化されるときにMutFuncRefにFuncRefがセットされる
    stub_globals: FxHashMap<usize, Global>,
    // instantiate_funcの結果を保存するグローバル
    instantiate_func_global: Option<Global>,
}

impl Jit {
    pub fn new(config: JitConfig) -> Self {
        Self {
            config,
            jit_module: TiVec::new(),
            closure_global_layout: ClosureGlobalLayout::new(),
            stub_globals: FxHashMap::default(),
            instantiate_func_global: None,
        }
    }

    pub fn config(&self) -> JitConfig {
        self.config
    }

    pub fn register_module(
        &mut self,
        global_manager: &mut GlobalManager,
        module: Module,
    ) -> Module {
        let module_id = self.jit_module.next_key();
        self.jit_module
            .push(JitModule::new(global_manager, module_id, module));
        self.jit_module[module_id].generate_stub_module(
            global_manager,
            &mut self.stub_globals,
            &mut self.instantiate_func_global,
        )
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
            self.instantiate_func_global.unwrap(),
            &mut self.closure_global_layout,
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
            self.config,
            module_id,
            func_id,
            func_index,
            bb_id,
            index,
            global_manager,
            self.instantiate_func_global.unwrap(),
            &mut self.closure_global_layout,
            &self.stub_globals,
        )
    }
}
