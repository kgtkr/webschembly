pub trait HasId {
    type Id;
    fn id(&self) -> Self::Id;
}
