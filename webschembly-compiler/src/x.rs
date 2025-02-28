use std::marker::PhantomData;

use frunk::labelled::ByNameFieldPlucker;
use frunk::HNil;

// trees that grow: https://github.com/guygastineau/rust-trees-that-grow/blob/main/src/lib.rs
pub trait Phase: Sized {
    type Prev: Phase;
}

pub trait FamilyX<X: Phase> {
    type X = X; // for macro
    type R: std::fmt::Debug + Clone;
    type RS: std::fmt::Debug + Clone;
}
pub type RunX<T, X> = <T as FamilyX<X>>::RS;

// FamilyX::RSの推奨される型
// マクロにしないと無限再帰と表示されて定義できないので
#[macro_export]
macro_rules! family_x_rs {
    () => {
        frunk::HCons<frunk::labelled::Field<Self::X, Self::R>, $crate::x::RunX<Self, <Self::X as Phase>::Prev>>
    };
}

#[derive(Debug, Clone)]
pub enum BasePhase {}
impl Phase for BasePhase {
    type Prev = BasePhase;
}

impl<T> FamilyX<BasePhase> for T
where
    T: std::fmt::Debug + Clone,
{
    type R = ();
    type RS = HNil;
}

pub fn by_phase<X, I, T: ByNameFieldPlucker<X, I>>(_x: PhantomData<X>, x: T) -> T::TargetValue {
    ByNameFieldPlucker::<X, I>::pluck_by_name(x).0.value
}
