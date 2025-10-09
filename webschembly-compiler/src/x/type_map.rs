use std::marker::PhantomData;

use frunk::{
    HCons, HNil, field,
    labelled::{ByNameFieldPlucker, Field},
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

// パラメータを取れるIntoトレイト
pub trait ElementInto<T> {
    type Param;

    fn element_into(self, param: Self::Param) -> T;
}

impl<T: Into<U>, U> ElementInto<U> for T {
    type Param = ();

    fn element_into(self, _: ()) -> U {
        self.into()
    }
}

pub trait IntoTypeMap<T2, P> {
    type Param;

    fn into_type_map(self, param: P) -> T2;
}

impl<P> IntoTypeMap<Empty, P> for Empty {
    type Param = ();

    fn into_type_map(self, _: P) -> Empty {
        self
    }
}

impl<K, V, T, V2, T2, P> IntoTypeMap<Add<K, V2, T2>, P> for Add<K, V, T>
where
    V: ElementInto<V2>,
    T: IntoTypeMap<T2, P>,
    P: Into<<V as ElementInto<V2>>::Param> + Clone,
{
    type Param = <V as ElementInto<V2>>::Param;

    fn into_type_map(self, param: P) -> Add<K, V2, T2> {
        self.tail
            .into_type_map(param.clone())
            .add(key::<K>(), self.head.value.element_into(param.into()))
    }
}
