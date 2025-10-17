use std::fmt;
use std::iter::from_coroutine;

use super::display::*;
use super::id::*;
use super::meta::*;
use ordered_float::NotNan;
use rustc_hash::FxHashMap;
use vec_map::{HasId, VecMap};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalFlag {
    Defined,
    Used(LocalUsedFlag),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalUsedFlag {
    Phi(BasicBlockId),
    NonPhi,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrCall {
    pub func_id: FuncId,
    pub args: Vec<LocalId>,
}

macro_rules! impl_InstrCall_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        yield &$($mutability)? self.func_id;
                    },
                )
            }
        }
    };
}

macro_rules! impl_InstrCall_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        for arg in &$($mutability)? self.args {
                            yield arg;
                        }
                    },
                )
            }
        }
    };
}

impl InstrCall {
    impl_InstrCall_func_ids!(_mut, mut);
    impl_InstrCall_func_ids!(,);
    impl_InstrCall_local_ids!(_mut, mut);
    impl_InstrCall_local_ids!(,);

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ InstrCall> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ InstrCall> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "call({})", self.value.func_id.display(self.meta.meta))?;
        if !self.value.args.is_empty() {
            write!(f, "(")?;
            for (i, arg) in self.value.args.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{}", arg.display(self.meta))?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrCallRef {
    pub func: LocalId,
    pub args: Vec<LocalId>,
    pub func_type: FuncType,
}

macro_rules! impl_InstrCallRef_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        yield &$($mutability)? self.func;
                        for arg in &$($mutability)? self.args {
                            yield arg;
                        }
                    },
                )
            }
        }
    };
}

impl InstrCallRef {
    impl_InstrCallRef_local_ids!(_mut, mut);
    impl_InstrCallRef_local_ids!(,);

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ InstrCallRef> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ InstrCallRef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "call_ref<{}>({})",
            self.value.func_type,
            self.value.func.display(self.meta),
        )?;
        if !self.value.args.is_empty() {
            write!(f, "(")?;
            for (i, arg) in self.value.args.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{}", arg.display(self.meta))?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

/*
以下に変換されるsyntax sugarのようなもの
JITの際に最適化しやすいように特殊な命令として実装している

l21 = closure_entrypoint_table(closure)
l22 = entrypoint_table_ref(index, l21)
l23 = deref_mut_func_ref(l22)
l0 = call_ref<(closure, ...arg_types) -> obj>(l23)(closure, ...args)
*/
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrCallClosure {
    pub closure: LocalId,
    pub args: Vec<LocalId>,
    pub arg_types: Vec<LocalType>,
    pub func_index: usize,
}

macro_rules! impl_InstrCallClosure_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        yield &$($mutability)? self.closure;
                        for arg in &$($mutability)? self.args {
                            yield arg;
                        }
                    },
                )
            }
        }
    };
}

impl InstrCallClosure {
    impl_InstrCallClosure_local_ids!(_mut, mut);
    impl_InstrCallClosure_local_ids!(,);

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ InstrCallClosure> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ InstrCallClosure> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "call_closure<")?;
        for (i, arg_type) in self.value.arg_types.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", arg_type)?;
        }
        write!(
            f,
            ">(closure={}, func_index={}",
            self.value.closure.display(self.meta),
            self.value.func_index
        )?;
        for arg in self.value.args.iter() {
            write!(f, ", {}", arg.display(self.meta))?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhiIncomingValue {
    pub bb: BasicBlockId,
    pub local: LocalId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstrKind {
    Nop,                        // 左辺はNoneでなければならない
    Phi(Vec<PhiIncomingValue>), // BBの先頭にのみ連続して出現可能(Nopが間に入るのは可)
    Uninitialized(LocalType), // // IR上で未初期化変数にアクセスすることを認めるとデータフロー解析などが複雑になるので、明示的に「未初期値」を表す命令を用意する
    InstantiateFunc(ModuleId, FuncId, usize),
    InstantiateClosureFunc(LocalId, LocalId, usize), // InstantiateFuncのModuleId/FuncIdを動的に指定する版
    InstantiateBB(ModuleId, FuncId, usize, BasicBlockId, usize),
    Bool(bool),
    Int(i64),
    Float(NotNan<f64>),
    NaN,
    String(String),
    StringToSymbol(LocalId),
    Nil,
    Char(char),
    Vector(Vec<LocalId>),
    UVector(UVectorKind, Vec<LocalId>),
    MakeUVector(UVectorKind, LocalId),
    Cons(LocalId, LocalId),
    CreateRef(Type),
    DerefRef(Type, LocalId),
    SetRef(Type, LocalId /* ref */, LocalId /* value */),
    FuncRef(FuncId),
    Call(InstrCall),
    CallRef(InstrCallRef),
    Closure {
        envs: Vec<LocalId>,
        module_id: ModuleId,
        func_id: FuncId,
        entrypoint_table: LocalId,
    },
    Move(LocalId),
    ToObj(ValType, LocalId),
    FromObj(ValType, LocalId),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        LocalId,        /* closure */
        usize,          /* env index */
    ),
    ClosureSetEnv(
        Vec<LocalType>, /* env types */
        LocalId,        /* closure */
        usize,          /* env index */
        LocalId,        /* value */
    ),
    ClosureModuleId(LocalId),        // (Closure) -> int
    ClosureFuncId(LocalId),          // (Closure) -> int
    ClosureEntrypointTable(LocalId), // (Closure) -> EntrypointTable
    GlobalSet(GlobalId, LocalId),
    GlobalGet(GlobalId),
    // builtins
    Display(LocalId),
    AddInt(LocalId, LocalId),
    AddFloat(LocalId, LocalId),
    SubInt(LocalId, LocalId),
    SubFloat(LocalId, LocalId),
    MulInt(LocalId, LocalId),
    MulFloat(LocalId, LocalId),
    DivInt(LocalId, LocalId),
    DivFloat(LocalId, LocalId),
    WriteChar(LocalId),
    Is(ValType, LocalId),
    VectorLength(LocalId),
    VectorRef(LocalId, LocalId),
    VectorSet(LocalId, LocalId, LocalId),
    UVectorLength(UVectorKind, LocalId),
    UVectorRef(UVectorKind, LocalId, LocalId),
    UVectorSet(UVectorKind, LocalId, LocalId, LocalId),
    EqObj(LocalId, LocalId),
    Not(LocalId),
    And(LocalId, LocalId),
    Or(LocalId, LocalId),
    Car(LocalId),
    Cdr(LocalId),
    SymbolToString(LocalId),
    IntToString(LocalId),
    FloatToString(LocalId),
    EqInt(LocalId, LocalId),
    EqFloat(LocalId, LocalId),
    LtInt(LocalId, LocalId),
    LtFloat(LocalId, LocalId),
    GtInt(LocalId, LocalId),
    GtFloat(LocalId, LocalId),
    LeInt(LocalId, LocalId),
    LeFloat(LocalId, LocalId),
    GeInt(LocalId, LocalId),
    GeFloat(LocalId, LocalId),
    VariadicArgs(Vec<LocalId>),
    VariadicArgsRef(LocalId, usize),
    VariadicArgsLength(LocalId),
    // ArgsVariadic(Vec<LocalId>, LocalId<Vector>)
    // ArgsRest(LocalId, usize) -> Vector
    CreateMutFuncRef(LocalId),                   // (FuncRef) -> MutFuncRef
    CreateEmptyMutFuncRef,                       // () -> MutFuncRef
    DerefMutFuncRef(LocalId),                    // (MutFuncRef) -> FuncRef
    SetMutFuncRef(LocalId, LocalId),             // (MutFuncRef, FuncRef) -> Nil
    EntrypointTable(Vec<LocalId>),               // (MutFuncRef...) -> EntrypointTable
    EntrypointTableRef(usize, LocalId),          // (EntrypointTable, index) -> MutFuncRef
    SetEntrypointTable(usize, LocalId, LocalId), // (EntrypointTable, index, MutFuncRef) -> Nil
    CallClosure(InstrCallClosure),
}

macro_rules! impl_InstrKind_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        InstrKind::FuncRef(func_id) => yield func_id,
                        InstrKind::Call(call) => {
                            for id in call.[<func_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        _ => {}
                    },
                )
            }
        }
    };
}

macro_rules! impl_InstrKind_local_usages {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_usages $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = (&$($mutability)? LocalId, LocalUsedFlag)> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        InstrKind::Phi(values) => {
                            for value in values.[<iter $($suffix)?>]() {
                                yield (&$($mutability)? value.local, LocalUsedFlag::Phi(value.bb));
                            }
                        }
                        InstrKind::Uninitialized(_) => {}
                        InstrKind::StringToSymbol(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::Vector(ids) => {
                            for id in ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::UVector(_, ids) => {
                            for id in ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::MakeUVector(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::Cons(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::DerefRef(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::SetRef(_, ref_id, value_id) => {
                            yield (ref_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::Call(call) => {
                            for id in call.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::Closure {
                            envs,
                            module_id: _,
                            func_id: _,
                            entrypoint_table,
                        } => {
                            for env in envs {
                                yield (env, LocalUsedFlag::NonPhi);
                            }
                            yield (entrypoint_table, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::CallRef(call_ref) => {
                            for id in call_ref.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::CallClosure(call_closure) => {
                            for id in call_closure.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::Move(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::ToObj(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::FromObj(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::ClosureEnv(_, closure, _) => yield (closure, LocalUsedFlag::NonPhi),
                        InstrKind::ClosureSetEnv(_, closure, _, value) => {
                            yield (closure, LocalUsedFlag::NonPhi);
                            yield (value, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::ClosureModuleId(closure) => yield (closure, LocalUsedFlag::NonPhi),
                        InstrKind::ClosureFuncId(closure) => yield (closure, LocalUsedFlag::NonPhi),
                        InstrKind::ClosureEntrypointTable(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::GlobalSet(_, value) => yield (value, LocalUsedFlag::NonPhi),
                        InstrKind::Display(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::AddInt(a, b)
                        | InstrKind::AddFloat(a, b)
                        | InstrKind::SubInt(a, b)
                        | InstrKind::SubFloat(a, b)
                        | InstrKind::MulInt(a, b)
                        | InstrKind::MulFloat(a, b)
                        | InstrKind::DivInt(a, b)
                        | InstrKind::DivFloat(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::WriteChar(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::Is(_,id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::VectorLength(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::VectorRef(vec_id, index_id) => {
                            yield (vec_id, LocalUsedFlag::NonPhi);
                            yield (index_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::VectorSet(vec_id, index_id, value_id) => {
                            yield (vec_id, LocalUsedFlag::NonPhi);
                            yield (index_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::UVectorLength(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::UVectorRef(_, uvec_id, index_id) => {
                            yield (uvec_id, LocalUsedFlag::NonPhi);
                            yield (index_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::UVectorSet(_, uvec_id, index_id, value_id) => {
                            yield (uvec_id, LocalUsedFlag::NonPhi);
                            yield (index_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::EqObj(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::Not(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::And(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::Or(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::Car(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::Cdr(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::SymbolToString(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::IntToString(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::FloatToString(id) => yield (id, LocalUsedFlag::NonPhi),
                        InstrKind::EqInt(a, b)
                        | InstrKind::EqFloat(a, b)
                        | InstrKind::LtInt(a, b)
                        | InstrKind::LtFloat(a, b)
                        | InstrKind::GtInt(a, b)
                        | InstrKind::GtFloat(a, b)
                        | InstrKind::LeInt(a, b)
                        | InstrKind::LeFloat(a, b)
                        | InstrKind::GeInt(a, b)
                        | InstrKind::GeFloat(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::VariadicArgs(ids) => {
                            for id in ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::VariadicArgsRef(id, _) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::VariadicArgsLength(id) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::CreateMutFuncRef(id) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::CreateEmptyMutFuncRef => {}
                        InstrKind::DerefMutFuncRef(id) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::SetMutFuncRef(mut_func_ref_id, func_ref_id) => {
                            yield (mut_func_ref_id, LocalUsedFlag::NonPhi);
                            yield (func_ref_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::EntrypointTable(mut_func_ref_ids) => {
                            for id in mut_func_ref_ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        InstrKind::EntrypointTableRef(_, entrypoint_table_id) => {
                            yield (entrypoint_table_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::SetEntrypointTable(_, entrypoint_table_id, mut_func_ref_id) => {
                            yield (entrypoint_table_id, LocalUsedFlag::NonPhi);
                            yield (mut_func_ref_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::InstantiateClosureFunc(module_id, func_id, _) => {
                            yield (module_id, LocalUsedFlag::NonPhi);
                            yield (func_id, LocalUsedFlag::NonPhi);
                        }


                        InstrKind::Nop
                        | InstrKind::InstantiateFunc(..)
                        | InstrKind::InstantiateBB(..)
                        | InstrKind::Bool(..)
                        | InstrKind::Int(..)
                        | InstrKind::Float(..)
                        | InstrKind::NaN
                        | InstrKind::String(..)
                        | InstrKind::Nil
                        | InstrKind::Char(..)
                        | InstrKind::CreateRef(..)
                        | InstrKind::FuncRef(..)
                        | InstrKind::GlobalGet(..) => {}
                    },
                )
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum InstrKindPurelity {
    Phi,
    // 決定的かつ副作用無し
    // 例: add
    Pure,
    // デッドコード削除可能だが、共通部分式除去はできない
    // 例: create_ref
    ImpureRead,
    // 副作用あり
    // 例: call
    Effectful,
}

impl InstrKindPurelity {
    // デッドコード削除可能か
    pub fn can_dce(&self) -> bool {
        match self {
            InstrKindPurelity::Pure | InstrKindPurelity::ImpureRead => true,
            InstrKindPurelity::Phi | InstrKindPurelity::Effectful => false,
        }
    }

    // 共通部分式除去可能か
    pub fn can_cse(&self) -> bool {
        match self {
            InstrKindPurelity::Pure => true,
            InstrKindPurelity::Phi
            | InstrKindPurelity::ImpureRead
            | InstrKindPurelity::Effectful => false,
        }
    }
}

impl InstrKind {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ InstrKind> {
        DisplayInFunc { value: self, meta }
    }

    pub fn purelity(&self) -> InstrKindPurelity {
        match self {
            InstrKind::Phi(..) => InstrKindPurelity::Phi,
            InstrKind::Nop
            | InstrKind::Uninitialized(_)
            | InstrKind::Bool(..)
            | InstrKind::Int(..)
            | InstrKind::Float(..)
            | InstrKind::NaN
            | InstrKind::Nil
            | InstrKind::Char(..)
            | InstrKind::FuncRef(..)
            | InstrKind::Move(..)
            | InstrKind::ToObj(..)
            | InstrKind::FromObj(..)
            // Closureのmodule_id/func_id/entrypoint_tableは不変である
            | InstrKind::ClosureModuleId(..)
            | InstrKind::ClosureFuncId(..)
            | InstrKind::ClosureEntrypointTable(..)
            | InstrKind::Is(..)
            | InstrKind::EqObj(..)
            | InstrKind::Not(..)
            | InstrKind::And(..)
            | InstrKind::Or(..)
            | InstrKind::EqInt(..)
            | InstrKind::EqFloat(..)
            | InstrKind::LtInt(..)
            | InstrKind::LtFloat(..)
            | InstrKind::GtInt(..)
            | InstrKind::GtFloat(..)
            | InstrKind::LeInt(..)
            | InstrKind::LeFloat(..)
            | InstrKind::GeInt(..)
            | InstrKind::GeFloat(..)
            | InstrKind::VariadicArgs(..)
            | InstrKind::VariadicArgsRef(..)
            | InstrKind::VariadicArgsLength(..)
            | InstrKind::AddInt(..)
            | InstrKind::AddFloat(..)
            | InstrKind::SubInt(..)
            | InstrKind::SubFloat(..)
            | InstrKind::MulInt(..)
            | InstrKind::MulFloat(..)
            | InstrKind::DivInt(..)
            | InstrKind::DivFloat(..) => InstrKindPurelity::Pure,
            // String/Cons/Vectorなどは可変なオブジェクトを生成するので純粋ではない
            InstrKind::String(..)
            | InstrKind::StringToSymbol(..)
            | InstrKind::Vector(..)
            | InstrKind::UVector(..)
            | InstrKind::MakeUVector(..)
            | InstrKind::Cons(..)
            | InstrKind::CreateRef(..)
            | InstrKind::DerefRef(..)
            | InstrKind::VectorLength(..)
            | InstrKind::VectorRef(..)
            | InstrKind::UVectorLength(..)
            | InstrKind::UVectorRef(..)
            | InstrKind::Car(..)
            | InstrKind::Cdr(..)
            | InstrKind::GlobalGet(..)
            | InstrKind::SymbolToString(..)
            | InstrKind::IntToString(..)
            | InstrKind::FloatToString(..)
            | InstrKind::CreateMutFuncRef(..)
            | InstrKind::CreateEmptyMutFuncRef
            | InstrKind::DerefMutFuncRef(..)
            | InstrKind::EntrypointTable(..)
            | InstrKind::EntrypointTableRef(..)
            | InstrKind::SetEntrypointTable(..)
            // closureの環境は可変である
            | InstrKind::Closure { .. }
            | InstrKind::ClosureEnv(..) => InstrKindPurelity::ImpureRead,
            InstrKind::InstantiateFunc(..)
            | InstrKind::InstantiateClosureFunc(..)
            | InstrKind::InstantiateBB(..)
            | InstrKind::SetRef(..)
            | InstrKind::SetMutFuncRef(..)
            | InstrKind::Call(..)
            | InstrKind::CallRef(..)
            | InstrKind::CallClosure(..)
            | InstrKind::GlobalSet(..)
            | InstrKind::Display(..)
            | InstrKind::WriteChar(..)
            | InstrKind::VectorSet(..)
            | InstrKind::UVectorSet(..)
            | InstrKind::ClosureSetEnv(..) => InstrKindPurelity::Effectful,
        }
    }

    impl_InstrKind_func_ids!(_mut, mut);
    impl_InstrKind_func_ids!(,);
    impl_InstrKind_local_usages!(_mut, mut);
    impl_InstrKind_local_usages!(,);
}

impl fmt::Display for DisplayInFunc<'_, &'_ InstrKind> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            InstrKind::Nop => write!(f, "nop"),
            InstrKind::Uninitialized(typ) => write!(f, "uninitialized<{}>", typ),
            InstrKind::Phi(values) => {
                write!(f, "phi(")?;
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(
                        f,
                        "{}: {}",
                        value.local.display(self.meta),
                        value.bb.display(self.meta.meta),
                    )?;
                }
                write!(f, ")")
            }
            InstrKind::InstantiateFunc(module_id, func_id, func_index) => {
                write!(
                    f,
                    "instantiate_func({}, {}, {})",
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta),
                    func_index
                )
            }
            InstrKind::InstantiateClosureFunc(module_id, func_id, func_index) => {
                write!(
                    f,
                    "instantiate_closure_func({}, {}, {})",
                    module_id.display(self.meta),
                    func_id.display(self.meta),
                    func_index
                )
            }
            InstrKind::InstantiateBB(module_id, func_id, func_index, bb_id, index) => {
                write!(
                    f,
                    "instantiate_bb({}, {}, {}, {}, {})",
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta),
                    func_index,
                    bb_id.display(self.meta.meta),
                    index,
                )
            }
            InstrKind::Bool(b) => write!(f, "{}", b),
            InstrKind::Int(i) => write!(f, "{}", i),
            InstrKind::Float(fl) => write!(f, "{}", fl),
            InstrKind::NaN => write!(f, "nan"),
            InstrKind::String(s) => write!(f, "{:?}", s),
            InstrKind::StringToSymbol(id) => {
                write!(f, "string_to_symbol({})", id.display(self.meta))
            }
            InstrKind::Nil => write!(f, "nil"),
            InstrKind::Char(c) => write!(f, "{:?}", c),
            InstrKind::Vector(v) => {
                write!(f, "[")?;
                for (i, id) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, "]")
            }
            InstrKind::UVector(kind, v) => {
                write!(f, "uvector<{}>[", kind)?;
                for (i, id) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, "]")
            }
            InstrKind::MakeUVector(kind, id) => {
                write!(f, "make_uvector<{}>({})", kind, id.display(self.meta))
            }
            InstrKind::Cons(a, b) => {
                write!(f, "({} . {})", a.display(self.meta), b.display(self.meta))
            }
            InstrKind::CreateRef(typ) => write!(f, "create_ref<{}>", typ),
            InstrKind::DerefRef(typ, id) => {
                write!(f, "deref_ref<{}>({})", typ, id.display(self.meta))
            }
            InstrKind::SetRef(typ, id, value) => {
                write!(
                    f,
                    "set_ref<{}>({}, {})",
                    typ,
                    id.display(self.meta),
                    value.display(self.meta)
                )
            }
            InstrKind::FuncRef(id) => write!(f, "func_ref({})", id.display(self.meta.meta)),
            InstrKind::Call(call) => {
                write!(f, "{}", call.display(self.meta))
            }
            InstrKind::Closure {
                envs,
                module_id,
                func_id,
                entrypoint_table,
            } => {
                write!(
                    f,
                    "closure(entrypoint_table={}, module_id={}, func_id={}",
                    entrypoint_table.display(self.meta),
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta)
                )?;
                for env in envs {
                    write!(f, ", {}", env.display(self.meta))?;
                }
                write!(f, ")")
            }
            InstrKind::CallRef(call_ref) => {
                write!(f, "{}", call_ref.display(self.meta))
            }
            InstrKind::CallClosure(call_closure) => {
                write!(f, "{}", call_closure.display(self.meta))
            }
            InstrKind::Move(id) => write!(f, "move({})", id.display(self.meta)),
            InstrKind::ToObj(typ, id) => write!(f, "to_obj<{}>({})", typ, id.display(self.meta)),
            InstrKind::FromObj(typ, id) => {
                write!(f, "from_obj<{}>({})", typ, id.display(self.meta))
            }
            InstrKind::ClosureEnv(env_types, closure, index) => {
                write!(f, "closure_env<")?;
                for (i, typ) in env_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", typ)?;
                }
                write!(f, ">({}, {})", closure.display(self.meta), index)?;
                Ok(())
            }
            InstrKind::ClosureSetEnv(env_types, closure, index, value) => {
                write!(f, "closure_set_env<")?;
                for (i, typ) in env_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", typ)?;
                }
                write!(
                    f,
                    ">({}, {}, {})",
                    closure.display(self.meta),
                    index,
                    value.display(self.meta)
                )?;
                Ok(())
            }
            InstrKind::ClosureModuleId(id) => {
                write!(f, "closure_module_id({})", id.display(self.meta))
            }
            InstrKind::ClosureFuncId(id) => write!(f, "closure_func_id({})", id.display(self.meta)),
            InstrKind::ClosureEntrypointTable(id) => {
                write!(f, "closure_entrypoint_table({})", id.display(self.meta))
            }
            InstrKind::GlobalSet(id, value) => {
                write!(
                    f,
                    "global_set({}, {})",
                    id.display(self.meta.meta),
                    value.display(self.meta)
                )
            }
            InstrKind::GlobalGet(id) => write!(f, "global_get({})", id.display(self.meta.meta)),
            InstrKind::Display(id) => write!(f, "display({})", id.display(self.meta)),
            InstrKind::AddInt(a, b) => {
                write!(
                    f,
                    "add_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::AddFloat(a, b) => {
                write!(
                    f,
                    "add_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::SubInt(a, b) => {
                write!(
                    f,
                    "sub_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::SubFloat(a, b) => {
                write!(
                    f,
                    "sub_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::MulInt(a, b) => {
                write!(
                    f,
                    "mul_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::MulFloat(a, b) => {
                write!(
                    f,
                    "mul_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::DivInt(a, b) => {
                write!(
                    f,
                    "div_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::DivFloat(a, b) => {
                write!(
                    f,
                    "div_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::WriteChar(id) => write!(f, "write_char({})", id.display(self.meta)),
            InstrKind::Is(typ, id) => write!(f, "is<{}>({})", typ, id.display(self.meta)),
            InstrKind::VectorLength(id) => write!(f, "vector_length({})", id.display(self.meta)),
            InstrKind::VectorRef(id, index) => {
                write!(
                    f,
                    "vector_ref({}, {})",
                    id.display(self.meta),
                    index.display(self.meta)
                )
            }
            InstrKind::VectorSet(id, index, value) => {
                write!(
                    f,
                    "vector_set({}, {}, {})",
                    id.display(self.meta),
                    index.display(self.meta),
                    value.display(self.meta)
                )
            }
            InstrKind::UVectorLength(kind, id) => {
                write!(f, "uvector_length<{}>({})", kind, id.display(self.meta))
            }
            InstrKind::UVectorRef(kind, uvec_id, index_id) => {
                write!(
                    f,
                    "uvector_ref<{}>({}, {})",
                    kind,
                    uvec_id.display(self.meta),
                    index_id.display(self.meta)
                )
            }
            InstrKind::UVectorSet(kind, uvec_id, index_id, value_id) => {
                write!(
                    f,
                    "uvector_set<{}>({}, {}, {})",
                    kind,
                    uvec_id.display(self.meta),
                    index_id.display(self.meta),
                    value_id.display(self.meta)
                )
            }
            InstrKind::EqObj(a, b) => {
                write!(
                    f,
                    "eq_obj({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::Not(id) => write!(f, "not({})", id.display(self.meta)),
            InstrKind::And(a, b) => {
                write!(f, "and({}, {})", a.display(self.meta), b.display(self.meta))
            }
            InstrKind::Or(a, b) => {
                write!(f, "or({}, {})", a.display(self.meta), b.display(self.meta))
            }
            InstrKind::Car(id) => write!(f, "car({})", id.display(self.meta)),
            InstrKind::Cdr(id) => write!(f, "cdr({})", id.display(self.meta)),
            InstrKind::SymbolToString(id) => {
                write!(f, "symbol_to_string({})", id.display(self.meta))
            }
            InstrKind::IntToString(id) => write!(f, "int_to_string({})", id.display(self.meta)),
            InstrKind::FloatToString(id) => write!(f, "float_to_string({})", id.display(self.meta)),
            InstrKind::EqInt(a, b) => write!(
                f,
                "eq_int({}, {})",
                a.display(self.meta),
                b.display(self.meta)
            ),
            InstrKind::EqFloat(a, b) => write!(
                f,
                "eq_float({}, {})",
                a.display(self.meta),
                b.display(self.meta)
            ),
            InstrKind::LtInt(a, b) => {
                write!(
                    f,
                    "lt_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::LtFloat(a, b) => {
                write!(
                    f,
                    "lt_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::GtInt(a, b) => {
                write!(
                    f,
                    "gt_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::GtFloat(a, b) => {
                write!(
                    f,
                    "gt_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::LeInt(a, b) => {
                write!(
                    f,
                    "le_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::LeFloat(a, b) => {
                write!(
                    f,
                    "le_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::GeInt(a, b) => {
                write!(
                    f,
                    "ge_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::GeFloat(a, b) => {
                write!(
                    f,
                    "ge_float({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::VariadicArgs(ids) => {
                write!(f, "variadic_args(")?;
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, ")")
            }
            InstrKind::VariadicArgsRef(id, index) => {
                write!(f, "variadic_args_ref({}, {})", id.display(self.meta), index)
            }
            InstrKind::VariadicArgsLength(id) => {
                write!(f, "variadic_args_length({})", id.display(self.meta))
            }
            InstrKind::CreateMutFuncRef(func_ref_id) => {
                write!(f, "create_mut_func_ref({})", func_ref_id.display(self.meta))
            }
            InstrKind::CreateEmptyMutFuncRef => write!(f, "create_empty_mut_func_ref"),
            InstrKind::DerefMutFuncRef(id) => {
                write!(f, "deref_mut_func_ref({})", id.display(self.meta))
            }
            InstrKind::SetMutFuncRef(mut_func_ref_id, func_ref_id) => write!(
                f,
                "set_mut_func_ref({}, {})",
                mut_func_ref_id.display(self.meta),
                func_ref_id.display(self.meta)
            ),
            InstrKind::EntrypointTable(mut_func_ref_ids) => {
                write!(f, "entrypoint_table(")?;
                for (i, id) in mut_func_ref_ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, ")")
            }
            InstrKind::EntrypointTableRef(index, entrypoint_table_id) => write!(
                f,
                "entrypoint_table_ref({}, {})",
                index,
                entrypoint_table_id.display(self.meta),
            ),
            InstrKind::SetEntrypointTable(index, entrypoint_table_id, mut_func_ref_id) => write!(
                f,
                "set_entrypoint_table({}, {}, {})",
                index,
                entrypoint_table_id.display(self.meta),
                mut_func_ref_id.display(self.meta)
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instr {
    pub local: Option<LocalId>,
    pub kind: InstrKind,
}

macro_rules! impl_Instr_local_usages {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_usages $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = (&$($mutability)? LocalId, LocalFlag)> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        for (id, used_flag) in self.kind.[<local_usages $($suffix)?>]() {
                            yield (id, LocalFlag::Used(used_flag));
                        }
                        if let Some(local) = &$($mutability)? self.local {
                            yield (local, LocalFlag::Defined);
                        }
                    },
                )
            }
        }
    };
}

impl Instr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ Instr> {
        DisplayInFunc { value: self, meta }
    }

    impl_Instr_local_usages!(_mut, mut);
    impl_Instr_local_usages!(,);
}

impl fmt::Display for DisplayInFunc<'_, &'_ Instr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(local) = self.value.local {
            write!(f, "{}", local.display(self.meta))?;
        } else {
            write!(f, "_")?;
        }
        write!(f, " = {}", self.value.kind.display(self.meta))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub instrs: Vec<Instr>,
    pub next: BasicBlockNext,
}

macro_rules! impl_BasicBlock_local_usages {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_usages $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = (&$($mutability)? LocalId, LocalFlag)> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        for instr in &$($mutability)? self.instrs {
                            for usage in instr.[<local_usages $($suffix)?>]() {
                                yield usage;
                            }
                        }
                        for id in self.next.[<local_ids $($suffix)?>]() {
                            yield (id, LocalFlag::Used(LocalUsedFlag::NonPhi));
                        }
                    },
                )
            }
        }
    };
}

macro_rules! impl_BasicBlock_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        for instr in &$($mutability)? self.instrs {
                            for id in instr.kind.[<func_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        for id in self.next.[<func_ids $($suffix)?>]() {
                            yield id;
                        }
                    },
                )
            }
        }
    };
}

impl BasicBlock {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ BasicBlock> {
        DisplayInFunc { value: self, meta }
    }

    impl_BasicBlock_local_usages!(_mut, mut);
    impl_BasicBlock_local_usages!(,);

    impl_BasicBlock_func_ids!(_mut, mut);
    impl_BasicBlock_func_ids!(,);
}

impl HasId for BasicBlock {
    type Id = BasicBlockId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ BasicBlock> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // きれいな実装ではないがインデントは決め打ちする
        writeln!(
            f,
            "{}{}:",
            DISPLAY_INDENT,
            self.value.id.display(self.meta.meta)
        )?;
        for instr in &self.value.instrs {
            writeln!(
                f,
                "{}{}{}",
                DISPLAY_INDENT,
                DISPLAY_INDENT,
                instr.display(self.meta)
            )?;
        }
        writeln!(
            f,
            "{}{}{}",
            DISPLAY_INDENT,
            DISPLAY_INDENT,
            self.value.next.display(self.meta)
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicBlockTerminator {
    Return(LocalId),
    TailCall(InstrCall),
    TailCallRef(InstrCallRef),
    TailCallClosure(InstrCallClosure),
    Error(LocalId),
}
macro_rules! impl_BasicBlockTerminator_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        BasicBlockTerminator::Return(local) => yield local,
                        BasicBlockTerminator::TailCall(call) => {
                            for id in call.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        BasicBlockTerminator::TailCallRef(call_ref) => {
                            for id in call_ref.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        BasicBlockTerminator::TailCallClosure(call_closure) => {
                            for id in call_closure.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        BasicBlockTerminator::Error(local) => yield local,
                    },
                )
            }
        }
    };
}

macro_rules! impl_BasicBlockTerminator_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        BasicBlockTerminator::Return(_)
                        | BasicBlockTerminator::TailCallRef(_)
                        | BasicBlockTerminator::TailCallClosure(_)
                        | BasicBlockTerminator::Error(_) => {}
                        BasicBlockTerminator::TailCall(call) => {
                            for id in call.[<func_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                    },
                )
            }
        }
    };
}

impl BasicBlockTerminator {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &BasicBlockTerminator> {
        DisplayInFunc { value: self, meta }
    }

    impl_BasicBlockTerminator_local_ids!(_mut, mut);
    impl_BasicBlockTerminator_local_ids!(,);

    impl_BasicBlockTerminator_func_ids!(_mut, mut);
    impl_BasicBlockTerminator_func_ids!(,);
}

impl fmt::Display for DisplayInFunc<'_, &BasicBlockTerminator> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            BasicBlockTerminator::Return(local) => write!(f, "return {}", local.display(self.meta)),
            BasicBlockTerminator::TailCall(call) => {
                write!(f, "tail_call {}", call.display(self.meta))
            }
            BasicBlockTerminator::TailCallRef(call_ref) => {
                write!(f, "tail_call_ref {}", call_ref.display(self.meta))
            }
            BasicBlockTerminator::TailCallClosure(call_closure) => {
                write!(f, "tail_call_closure {}", call_closure.display(self.meta))
            }
            BasicBlockTerminator::Error(local) => write!(f, "error {}", local.display(self.meta)),
        }
    }
}

// 閉路を作ってはいけない
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicBlockNext {
    If(LocalId, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Terminator(BasicBlockTerminator),
}

macro_rules! impl_BasicBlockNext_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        BasicBlockNext::If(cond, _, _) => yield cond,
                        BasicBlockNext::Jump(_) => {}
                        BasicBlockNext::Terminator(terminator) => {
                            for id in terminator.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                    },
                )
            }
        }
    };
}

macro_rules! impl_BasicBlockNext_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        BasicBlockNext::If(_, _, _)
                        | BasicBlockNext::Jump(_) => {}
                        BasicBlockNext::Terminator(terminator) => {
                            for id in terminator.[<func_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                    },
                )
            }
        }
    };
}

impl BasicBlockNext {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &BasicBlockNext> {
        DisplayInFunc { value: self, meta }
    }

    impl_BasicBlockNext_local_ids!(_mut, mut);
    impl_BasicBlockNext_local_ids!(,);

    impl_BasicBlockNext_func_ids!(_mut, mut);
    impl_BasicBlockNext_func_ids!(,);
}

impl fmt::Display for DisplayInFunc<'_, &BasicBlockNext> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            BasicBlockNext::If(cond, bb1, bb2) => {
                write!(
                    f,
                    "if {} then {} else {}",
                    cond.display(self.meta),
                    bb1.display(self.meta.meta),
                    bb2.display(self.meta.meta)
                )
            }
            BasicBlockNext::Jump(bb) => write!(f, "jump {}", bb.display(self.meta.meta)),
            BasicBlockNext::Terminator(terminator) => {
                write!(f, "{}", terminator.display(self.meta))
            }
        }
    }
}

impl BasicBlockNext {
    pub fn successors(&self) -> impl Iterator<Item = BasicBlockId> {
        from_coroutine(
            #[coroutine]
            move || match self {
                BasicBlockNext::If(_, t, f) => {
                    yield *t;
                    yield *f;
                }
                BasicBlockNext::Jump(bb) => {
                    yield *bb;
                }
                BasicBlockNext::Terminator(_) => {}
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    pub id: LocalId,
    pub typ: LocalType,
}

impl HasId for Local {
    type Id = LocalId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Local {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ Local> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ Local> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "local {}: {}",
            self.value.id.display(self.meta),
            self.value.typ
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Func {
    pub id: FuncId,
    pub locals: VecMap<LocalId, Local>,
    pub args: Vec<LocalId>,
    pub ret_type: LocalType,
    pub bb_entry: BasicBlockId,
    pub bbs: VecMap<BasicBlockId, BasicBlock>,
}

impl HasId for Func {
    type Id = FuncId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Func {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, &'_ Func> {
        Display { value: self, meta }
    }

    pub fn arg_types(&self) -> Vec<LocalType> {
        self.args.iter().map(|&arg| self.locals[arg].typ).collect()
    }

    pub fn ret_type(&self) -> LocalType {
        self.ret_type
    }

    pub fn func_type(&self) -> FuncType {
        FuncType {
            args: self.arg_types(),
            ret: self.ret_type(),
        }
    }
}

impl fmt::Display for Display<'_, &'_ Func> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.value.id.display(self.meta))?;
        for local in self.value.locals.values() {
            writeln!(
                f,
                "{}{}",
                DISPLAY_INDENT,
                local.display(self.meta.in_func(self.value.id))
            )?;
        }
        write!(f, "{}args: ", DISPLAY_INDENT)?;
        for (i, arg) in self.value.args.iter().enumerate() {
            write!(f, "{}", arg.display(self.meta.in_func(self.value.id)))?;
            if i < self.value.args.len() - 1 {
                write!(f, ",")?;
            }
        }
        writeln!(f)?;
        writeln!(f, "{}ret_type: {}", DISPLAY_INDENT, self.value.ret_type)?;
        writeln!(
            f,
            "{}entry: {}",
            DISPLAY_INDENT,
            self.value.bb_entry.display(self.meta)
        )?;
        for bb in self.value.bbs.values() {
            write!(f, "{}", bb.display(self.meta.in_func(self.value.id)))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub args: Vec<LocalType>,
    pub ret: LocalType,
}

impl std::fmt::Display for FuncType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", arg)?;
        }
        write!(f, ") -> {}", self.ret)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Global {
    pub id: GlobalId,
    pub typ: LocalType,
    pub linkage: GlobalLinkage,
}

impl Global {
    pub fn to_import(self) -> Self {
        Self {
            linkage: GlobalLinkage::Import,
            ..self
        }
    }

    pub fn to_export(self) -> Self {
        Self {
            linkage: GlobalLinkage::Export,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalLinkage {
    Import,
    Export,
}

impl HasId for Global {
    type Id = GlobalId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Global {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, &'_ Global> {
        Display { value: self, meta }
    }
}

impl fmt::Display for Display<'_, &'_ Global> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let linkage = match self.value.linkage {
            GlobalLinkage::Import => "import",
            GlobalLinkage::Export => "export",
        };
        write!(
            f,
            "{} global {}: {}",
            linkage,
            self.value.id.display(self.meta),
            self.value.typ
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub globals: FxHashMap<GlobalId, Global>,
    pub funcs: VecMap<FuncId, Func>,
    pub entry: FuncId,
    pub meta: Meta,
}

impl Module {
    pub fn display(&self) -> Display<'_, &Module> {
        Display {
            value: self,
            meta: &self.meta,
        }
    }
}
impl fmt::Display for Display<'_, &'_ Module> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for global in self.value.globals.values() {
            writeln!(f, "{}", global.display(self.meta))?;
        }

        writeln!(f, "entry: {}", self.value.entry.display(self.meta))?;
        for func in self.value.funcs.values() {
            write!(f, "{}", func.display(self.meta))?;
        }
        Ok(())
    }
}
