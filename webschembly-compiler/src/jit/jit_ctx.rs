use rustc_hash::FxHashMap;

use super::jit_config::JitConfig;
use super::jit_module::ClosureGlobalLayout;
use crate::ir::*;

#[derive(Debug)]
pub struct JitCtx {
    config: JitConfig,
    closure_global_layout: ClosureGlobalLayout,
    // 1つ以上のモジュールがインスタンス化されているか
    is_instantiated: bool,
    // 0..GLOBAL_LAYOUT_MAX_SIZEまでのindexに対応する関数のスタブが入ったMutFuncRef
    // func_indexがインスタンス化されるときにMutFuncRefにFuncRefがセットされる
    stub_globals: FxHashMap<usize, Global>,
    // instantiate_funcの結果を保存するグローバル
    instantiate_func_global: Option<Global>,
}

impl JitCtx {
    pub fn new(config: JitConfig) -> Self {
        Self {
            config,
            closure_global_layout: ClosureGlobalLayout::new(),
            is_instantiated: false,
            stub_globals: FxHashMap::default(),
            instantiate_func_global: None,
        }
    }

    pub fn config(&self) -> JitConfig {
        self.config
    }

    pub fn stub_global(&self, index: usize) -> Global {
        debug_assert!(self.is_instantiated);
        self.stub_globals[&index]
    }

    pub fn instantiate_func_global(&self) -> Global {
        debug_assert!(self.is_instantiated);
        self.instantiate_func_global.unwrap()
    }

    pub fn is_instantiated(&self) -> bool {
        self.is_instantiated
    }

    pub fn init_instantiated(
        &mut self,
        stub_globals: FxHashMap<usize, Global>,
        instantiate_func_global: Global,
    ) {
        debug_assert!(!self.is_instantiated);
        debug_assert!(self.stub_globals.is_empty());
        debug_assert!(self.instantiate_func_global.is_none());

        self.is_instantiated = true;
        self.stub_globals = stub_globals;
        self.instantiate_func_global = Some(instantiate_func_global);
    }

    pub fn closure_global_layout(&mut self) -> &mut ClosureGlobalLayout {
        &mut self.closure_global_layout
    }
}
