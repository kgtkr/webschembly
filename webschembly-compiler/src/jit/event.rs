#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitLogEvent {
    BasicBlock {
        module_id: usize,
        func_id: usize,
        env_index: usize,
        func_index: usize,
        bb_id: usize,
        index: usize,
        successors: Vec<usize>,
    },
}
