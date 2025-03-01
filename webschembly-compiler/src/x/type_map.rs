use std::marker::PhantomData;

use frunk::{
    field,
    hlist::HList,
    labelled::{ByNameFieldPlucker, Field},
    HCons, HNil,
};

use super::by_name_field_ref_plucker::ByNameFieldRefPlucker;

pub trait TypeMap {
    fn get_owned<K, I>(self, _k: Key<K>) -> <Self as ByNameFieldPlucker<K, I>>::TargetValue
    where
        Self: ByNameFieldPlucker<K, I>;

    fn get_ref<K, I>(&self, _k: Key<K>) -> &<Self as ByNameFieldRefPlucker<K, I>>::TargetValue
    where
        Self: ByNameFieldRefPlucker<K, I>;

    fn add<K, V>(self, _k: Key<K>, value: V) -> HCons<Field<K, V>, Self>
    where
        Self: HList;
}

impl<T> TypeMap for T {
    fn get_owned<K, I>(self, _k: Key<K>) -> <Self as ByNameFieldPlucker<K, I>>::TargetValue
    where
        Self: ByNameFieldPlucker<K, I>,
    {
        ByNameFieldPlucker::<K, I>::pluck_by_name(self).0.value
    }

    fn get_ref<K, I>(&self, _k: Key<K>) -> &<Self as ByNameFieldRefPlucker<K, I>>::TargetValue
    where
        Self: ByNameFieldRefPlucker<K, I>,
    {
        ByNameFieldRefPlucker::<K, I>::ref_pluck_by_name(self)
            .0
            .value
    }

    fn add<K, V>(self, _k: Key<K>, value: V) -> Add<K, V, Self>
    where
        Self: HList,
    {
        HCons {
            head: field![K, value],
            tail: self,
        }
    }
}

pub type Key<K> = PhantomData<K>;
pub type Empty = HNil;
pub type Singleton<K, V> = HCons<Field<K, V>, Empty>;
pub type Add<K, V, T> = HCons<Field<K, V>, T>;

pub fn key<K>() -> Key<K> {
    PhantomData
}

pub fn singleton<K, V>(_k: Key<K>, value: V) -> Singleton<K, V> {
    HNil.add(_k, value)
}

pub fn empty() -> Empty {
    HNil
}
