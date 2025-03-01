use std::marker::PhantomData;

use frunk::labelled::{ByNameFieldPlucker, Field};
use frunk::{HCons, HNil};

use crate::by_name_field_ref_plucker::ByNameFieldRefPlucker;

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
    type R = HCons<Field<X, <T as FamilyX<X>>::R>, <T as FamilyRunX<<X as Phase>::Prev>>::R>;
}

impl<T> FamilyRunX<BasePhase> for T {
    type R = HNil;
}

pub type RunX<T, X> = <T as FamilyRunX<X>>::R;

#[derive(Debug, Clone)]
pub enum BasePhase {}

pub fn by_phase<X, I, T: ByNameFieldPlucker<X, I>>(_x: PhantomData<X>, x: T) -> T::TargetValue {
    ByNameFieldPlucker::<X, I>::pluck_by_name(x).0.value
}

pub fn by_phase_ref<X, I, T: ByNameFieldRefPlucker<X, I>>(
    _x: PhantomData<X>,
    x: &T,
) -> &T::TargetValue {
    ByNameFieldRefPlucker::<X, I>::ref_pluck_by_name(x).0.value
}
