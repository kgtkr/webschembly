use refl::Id;
use std::marker::PhantomData;

// T = MutCellC<_>
pub struct IdMutCellC<T: ?Sized>(PhantomData<(fn(T) -> T)>);

pub const fn refl_mut_cell<T: ?Sized>() -> IdMutCellC<MutCellC<T>> {
    IdMutCellC(PhantomData)
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
