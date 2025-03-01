use std::marker::PhantomData;

use frunk::{
    field,
    labelled::{ByNameFieldPlucker, Field},
    HCons, HNil,
};

pub trait TypeMap: Sized {
    fn get_owned<K, I>(self, _k: Key<K>) -> <Self as ByNameFieldPlucker<K, I>>::TargetValue
    where
        Self: ByNameFieldPlucker<K, I>;

    fn get_ref<'a, K, I>(
        &'a self,
        _k: Key<K>,
    ) -> <&'a Self as ByNameFieldPlucker<K, I>>::TargetValue
    where
        &'a Self: ByNameFieldPlucker<K, I>;

    fn add<K, V>(self, _k: Key<K>, value: V) -> HCons<Field<K, V>, Self>;
}

impl<T> TypeMap for T {
    fn get_owned<K, I>(self, _k: Key<K>) -> <Self as ByNameFieldPlucker<K, I>>::TargetValue
    where
        Self: ByNameFieldPlucker<K, I>,
    {
        ByNameFieldPlucker::<K, I>::pluck_by_name(self).0.value
    }

    fn get_ref<'a, K, I>(
        &'a self,
        _k: Key<K>,
    ) -> <&'a Self as ByNameFieldPlucker<K, I>>::TargetValue
    where
        &'a Self: ByNameFieldPlucker<K, I>,
    {
        ByNameFieldPlucker::<K, I>::pluck_by_name(self).0.value
    }

    fn add<K, V>(self, _k: Key<K>, value: V) -> Add<K, V, Self> {
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

pub trait IntoTypeMap<T2> {
    fn into_type_map(self) -> T2;
}

impl IntoTypeMap<Empty> for Empty {
    fn into_type_map(self) -> Empty {
        self
    }
}

impl<K, V, T, V2, T2> IntoTypeMap<Add<K, V2, T2>> for Add<K, V, T>
where
    V: Into<V2>,
    T: IntoTypeMap<T2>,
{
    fn into_type_map(self) -> Add<K, V2, T2> {
        self.tail
            .into_type_map()
            .add(key::<K>(), self.head.value.into())
    }
}
