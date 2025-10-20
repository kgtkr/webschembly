use super::id::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalFlag {
    Defined,
    Used(LocalUsedFlag),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalUsedFlag {
    Phi(BasicBlockId),
    NonPhi,
}
