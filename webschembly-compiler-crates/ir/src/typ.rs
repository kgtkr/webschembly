/*
TypeもしくはmutableなTypeを表す
Type自体にRefを含めて再帰的にしてしまうと無限種類の型を作れるようになってしまうので、IRではそれを避けるためこのような構造になっている
TODO: LocalTypeという名前は適切ではない
*/
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum LocalType {
    #[display("ref<{}>", _0)]
    Ref(Type), // TODO: Ref<Obj>固定で良いかも？
    #[display("{}", _0)]
    Type(Type),
    #[display("variadic_args")]
    VariadicArgs,
    #[display("mut_func_ref")]
    MutFuncRef,
    #[display("entrypoint_table")]
    EntrypointTable,
    #[display("func_ref")]
    FuncRef,
}

impl From<Type> for LocalType {
    fn from(typ: Type) -> Self {
        Self::Type(typ)
    }
}

impl From<ValType> for LocalType {
    fn from(typ: ValType) -> Self {
        Self::Type(Type::from(typ))
    }
}

impl LocalType {
    pub fn to_type(&self) -> Option<Type> {
        match self {
            LocalType::Type(typ) => Some(*typ),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum Type {
    #[display("obj")]
    Obj,
    #[display("{}", _0)]
    Val(ValType),
}

impl Type {
    pub fn to_val_type(&self) -> Option<ValType> {
        match self {
            Type::Val(val_type) => Some(*val_type),
            Type::Obj => None,
        }
    }
}

impl From<ValType> for Type {
    fn from(typ: ValType) -> Self {
        Self::Val(typ)
    }
}

// Objにアップキャスト可能な型
// 基本的にSchemeの型に対応するがFuncRefなど例外もある
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum ValType {
    #[display("nil")]
    Nil,
    #[display("bool")]
    Bool,
    #[display("char")]
    Char,
    #[display("int")]
    Int,
    #[display("float")]
    Float,
    #[display("string")]
    String,
    #[display("symbol")]
    Symbol,
    #[display("cons")]
    Cons,
    #[display("vector")]
    Vector,
    #[display("uvector<{0}>", _0)]
    UVector(UVectorKind),
    #[display("closure")]
    Closure,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum UVectorKind {
    #[display("s64")]
    S64,
    #[display("f64")]
    F64,
}

impl UVectorKind {
    pub fn element_type(&self) -> ValType {
        match self {
            UVectorKind::S64 => ValType::Int,
            UVectorKind::F64 => ValType::Float,
        }
    }
}
