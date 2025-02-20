// trees that grow: https://github.com/guygastineau/rust-trees-that-grow/blob/main/src/lib.rs
pub trait FamilyX<X> {
    type R: std::fmt::Debug + Clone;
}
pub type RunX<T, X> = <T as FamilyX<X>>::R;
