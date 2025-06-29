use derive_where::derive_where;
use std::marker::PhantomData;

// 型消去された型を表す
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Erased;

// refl crateのIdと似たようなもの
// ただし、Id<T, T>だけでなくId<S, Erased>の値も存在する
// castは不可能
#[derive_where(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeEq<S: ?Sized, T: ?Sized>(PhantomData<(fn(S) -> S, fn(T) -> T)>);

impl<S: ?Sized, T: ?Sized> TypeEq<S, T> {
    pub fn erase(self) -> TypeEq<S, Erased> {
        TypeEq(PhantomData)
    }
}

pub fn type_eq<T: ?Sized>() -> TypeEq<T, T> {
    TypeEq(PhantomData)
}

pub trait LocalTypeS {}

pub struct MutCellC<T: TypeS>(T);
impl<T: TypeS> LocalTypeS for MutCellC<T> {}

pub struct TypeC<T: TypeS>(T);
impl<T: TypeS> LocalTypeS for TypeC<T> {}

pub trait TypeS {}

pub struct BoxedC;
impl TypeS for BoxedC {}

pub struct ValC<T: ValTypeS>(T);
impl<T: ValTypeS> TypeS for ValC<T> {}

pub trait ValTypeS {}

pub struct BoolC;
impl ValTypeS for BoolC {}

pub struct IntC;
impl ValTypeS for IntC {}

pub struct StringC;
impl ValTypeS for StringC {}

pub struct SymbolC;
impl ValTypeS for SymbolC {}

pub struct NilC;
impl ValTypeS for NilC {}

pub struct ConsC;
impl ValTypeS for ConsC {}

pub struct ClosureC;
impl ValTypeS for ClosureC {}

pub struct CharC;
impl ValTypeS for CharC {}

pub struct VectorC;
impl ValTypeS for VectorC {}

pub struct FuncRefC;
impl ValTypeS for FuncRefC {}
