use std::marker::PhantomData;

use frunk::{
    indices::{Here, There},
    labelled::Field,
    HCons,
};

// ByNameFieldPlucker の参照を取得する版

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub struct FieldRef<'a, Name, Type> {
    name_type_holder: PhantomData<Name>,
    pub name: &'static str,
    pub value: &'a Type,
}

fn field_ref_with_name<'a, Label, Value>(
    name: &'static str,
    value: &'a Value,
) -> FieldRef<'a, Label, Value> {
    FieldRef {
        name_type_holder: PhantomData,
        name,
        value,
    }
}

pub trait ByNameFieldRefPlucker<TargetKey, Index> {
    type TargetValue;
    type Remainder<'a>
    where
        Self: 'a;

    /// Returns a pair consisting of the value pointed to by the target key and the remainder.
    fn ref_pluck_by_name<'a>(
        &'a self,
    ) -> (
        FieldRef<'a, TargetKey, Self::TargetValue>,
        Self::Remainder<'a>,
    );
}

/// Implementation when the pluck target key is in the head.
impl<K, V, Tail> ByNameFieldRefPlucker<K, Here> for HCons<Field<K, V>, Tail> {
    type TargetValue = V;
    type Remainder<'a>
        = &'a Tail
    where
        K: 'a,
        V: 'a,
        Tail: 'a;

    #[inline(always)]
    fn ref_pluck_by_name<'a>(
        &'a self,
    ) -> (FieldRef<'a, K, Self::TargetValue>, Self::Remainder<'a>) {
        let field = field_ref_with_name(self.head.name, &self.head.value);
        (field, &self.tail)
    }
}

/// Implementation when the pluck target key is in the tail.
impl<Head, Tail, K, TailIndex> ByNameFieldRefPlucker<K, There<TailIndex>> for HCons<Head, Tail>
where
    Tail: ByNameFieldRefPlucker<K, TailIndex>,
{
    type TargetValue = <Tail as ByNameFieldRefPlucker<K, TailIndex>>::TargetValue;
    type Remainder<'a>
        = HCons<&'a Head, <Tail as ByNameFieldRefPlucker<K, TailIndex>>::Remainder<'a>>
    where
        Tail: 'a,
        Head: 'a;

    #[inline(always)]
    fn ref_pluck_by_name<'a>(
        &'a self,
    ) -> (FieldRef<'a, K, Self::TargetValue>, Self::Remainder<'a>) {
        let (target, tail_remainder) =
            <Tail as ByNameFieldRefPlucker<K, TailIndex>>::ref_pluck_by_name(&self.tail);
        (
            target,
            HCons {
                head: &self.head,
                tail: tail_remainder,
            },
        )
    }
}
