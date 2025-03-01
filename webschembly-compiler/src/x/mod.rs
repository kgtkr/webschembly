mod by_name_field_ref_plucker;

pub mod type_map;
pub use type_map::TypeMap;

// trees that grow: https://github.com/guygastineau/rust-trees-that-grow/blob/main/src/lib.rs
pub trait Phase: Sized {
    type Prev;
}

pub trait FamilyX<X: Phase> {
    type R: std::fmt::Debug + Clone;
}
pub trait FamilyRunX<X> {
    type R: std::fmt::Debug + Clone;
}

impl<T: FamilyX<X>, X: Phase> FamilyRunX<X> for T
where
    T: FamilyRunX<<X as Phase>::Prev>,
    T: FamilyX<X>,
    X: Clone,
{
    type R = type_map::Add<X, <T as FamilyX<X>>::R, <T as FamilyRunX<<X as Phase>::Prev>>::R>;
}

impl<T> FamilyRunX<BasePhase> for T {
    type R = type_map::Empty;
}

pub type RunX<T, X> = <T as FamilyRunX<X>>::R;

#[derive(Debug, Clone)]
pub enum BasePhase {}
