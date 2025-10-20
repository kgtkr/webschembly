#[derive(Debug, Clone, Copy)]
pub struct JitConfig {
    pub enable_optimization: bool,
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
        }
    }
}
