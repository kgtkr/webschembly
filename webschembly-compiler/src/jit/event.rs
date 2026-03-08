use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum JitLogEvent {
    #[serde(rename = "bb")]
    BasicBlock {
        module_id: usize,
        func_id: usize,
        env_index: usize,
        func_index: usize,
        bb_id: usize,
        index: usize,
        successors: Vec<(usize, usize)>,
        display: String,
    },
}
