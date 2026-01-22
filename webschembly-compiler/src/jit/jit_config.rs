#[derive(Debug, Clone, Copy)]
pub struct JitConfig {
    pub enable_optimization: bool,
    pub block_fusion: BlockFusionConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFusionConfig {
    Disabled,
    SmallFusion,
    LargeFusion,
}

impl Default for JitConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl JitConfig {
    pub const fn new() -> Self {
        Self {
            enable_optimization: true,
            block_fusion: BlockFusionConfig::LargeFusion,
        }
    }
}
