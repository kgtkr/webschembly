use std::{fmt, iter::from_coroutine};

use derive_more::{From, Into};
use ordered_float::NotNan;
use rustc_hash::FxHashMap;
use strum_macros::EnumIter;
use typed_index_collections::TiVec;

use crate::{HasId, vec_map::VecMap};

const DISPLAY_INDENT: &str = "  ";

#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
}
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub local_metas: FxHashMap<(FuncId, LocalId), VarMeta>,
    pub global_metas: FxHashMap<GlobalId, VarMeta>,
}

impl Meta {
    pub fn in_func<'a>(&'a self, func_id: FuncId) -> MetaInFunc<'a> {
        MetaInFunc {
            func_id,
            meta: self,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Display<'a, T> {
    value: T,
    meta: &'a Meta,
}

#[derive(Debug, Clone)]
pub struct DisplayInFunc<'a, T> {
    value: T,
    meta: MetaInFunc<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct MetaInFunc<'a> {
    func_id: FuncId,
    meta: &'a Meta,
}

/*
TypeもしくはmutableなTypeを表す
Type自体にRefを含めて再帰的にしてしまうと無限種類の型を作れるようになってしまうので、IRではそれを避けるためこのような構造になっている
TODO: LocalTypeという名前は適切ではない
*/
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum LocalType {
    #[display("ref({})", _0)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display, EnumIter)]
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
    #[display("closure")]
    Closure,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct LocalId(usize);

impl LocalId {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, LocalId> {
        DisplayInFunc { value: *self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, LocalId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "l{}", self.value.0)?;
        if let Some(meta) = self
            .meta
            .meta
            .local_metas
            .get(&(self.meta.func_id, self.value))
        {
            write!(f, "_{}", meta.name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct GlobalId(usize);

impl GlobalId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, GlobalId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, GlobalId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "g{}", self.value.0)?;
        if let Some(meta) = self.meta.global_metas.get(&self.value) {
            write!(f, "_{}", meta.name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct FuncId(usize);

impl FuncId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, FuncId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, FuncId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "f{}", self.value.0)?;
        // TODO: add function name

        Ok(())
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
pub struct ExprCall {
    pub func_id: FuncId,
    pub args: Vec<LocalId>,
}

macro_rules! impl_ExprCall_func_ids {
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

macro_rules! impl_ExprCall_local_ids {
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

impl ExprCall {
    impl_ExprCall_func_ids!(_mut, mut);
    impl_ExprCall_func_ids!(,);
    impl_ExprCall_local_ids!(_mut, mut);
    impl_ExprCall_local_ids!(,);

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprCall> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprCall> {
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
pub struct ExprCallRef {
    pub func: LocalId,
    pub args: Vec<LocalId>,
    pub func_type: FuncType,
}

macro_rules! impl_ExprCallRef_local_ids {
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

impl ExprCallRef {
    impl_ExprCallRef_local_ids!(_mut, mut);
    impl_ExprCallRef_local_ids!(,);

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprCallRef> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprCallRef> {
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
pub struct ExprCallClosure {
    pub closure: LocalId,
    pub args: Vec<LocalId>,
    pub arg_types: Vec<LocalType>,
    pub func_index: usize,
}

macro_rules! impl_ExprCallClosure_local_ids {
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

impl ExprCallClosure {
    impl_ExprCallClosure_local_ids!(_mut, mut);
    impl_ExprCallClosure_local_ids!(,);

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprCallClosure> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprCallClosure> {
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
pub enum Expr {
    Nop,                        // 左辺はNoneでなければならない
    Phi(Vec<PhiIncomingValue>), // BBの先頭にのみ連続して出現可能(Nopが間に入るのは可)
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
    Cons(LocalId, LocalId),
    CreateRef(Type),
    DerefRef(Type, LocalId),
    SetRef(Type, LocalId /* ref */, LocalId /* value */),
    FuncRef(FuncId),
    Call(ExprCall),
    CallRef(ExprCallRef),
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
    ClosureModuleId(LocalId),        // (Closure) -> int
    ClosureFuncId(LocalId),          // (Closure) -> int
    ClosureEntrypointTable(LocalId), // (Closure) -> EntrypointTable
    GlobalSet(GlobalId, LocalId),
    GlobalGet(GlobalId),
    // builtins
    Display(LocalId),
    AddInt(LocalId, LocalId),
    SubInt(LocalId, LocalId),
    MulInt(LocalId, LocalId),
    DivInt(LocalId, LocalId),
    WriteChar(LocalId),
    Is(ValType, LocalId),
    VectorLength(LocalId),
    VectorRef(LocalId, LocalId),
    VectorSet(LocalId, LocalId, LocalId),
    EqObj(LocalId, LocalId),
    Not(LocalId),
    And(LocalId, LocalId),
    Or(LocalId, LocalId),
    Car(LocalId),
    Cdr(LocalId),
    SymbolToString(LocalId),
    NumberToString(LocalId),
    EqNum(LocalId, LocalId),
    LtInt(LocalId, LocalId),
    GtInt(LocalId, LocalId),
    LeInt(LocalId, LocalId),
    GeInt(LocalId, LocalId),
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
    CallClosure(ExprCallClosure),
}

macro_rules! impl_Expr_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        Expr::FuncRef(func_id) => yield func_id,
                        Expr::Call(call) => {
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

macro_rules! impl_Expr_local_usages {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_usages $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = (&$($mutability)? LocalId, LocalUsedFlag)> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        Expr::Phi(values) => {
                            for value in values.[<iter $($suffix)?>]() {
                                yield (&$($mutability)? value.local, LocalUsedFlag::Phi(value.bb));
                            }
                        }
                        Expr::StringToSymbol(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::Vector(ids) => {
                            for id in ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        Expr::Cons(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::DerefRef(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::SetRef(_, ref_id, value_id) => {
                            yield (ref_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
                        Expr::Call(call) => {
                            for id in call.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        Expr::Closure {
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
                        Expr::CallRef(call_ref) => {
                            for id in call_ref.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        Expr::CallClosure(call_closure) => {
                            for id in call_closure.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        Expr::Move(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::ToObj(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::FromObj(_, id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::ClosureEnv(_, closure, _) => yield (closure, LocalUsedFlag::NonPhi),
                        Expr::ClosureModuleId(closure) => yield (closure, LocalUsedFlag::NonPhi),
                        Expr::ClosureFuncId(closure) => yield (closure, LocalUsedFlag::NonPhi),
                        Expr::ClosureEntrypointTable(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::GlobalSet(_, value) => yield (value, LocalUsedFlag::NonPhi),
                        Expr::Display(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::AddInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::SubInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::MulInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::DivInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::WriteChar(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::Is(_,id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::VectorLength(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::VectorRef(vec_id, index_id) => {
                            yield (vec_id, LocalUsedFlag::NonPhi);
                            yield (index_id, LocalUsedFlag::NonPhi);
                        }
                        Expr::VectorSet(vec_id, index_id, value_id) => {
                            yield (vec_id, LocalUsedFlag::NonPhi);
                            yield (index_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
                        Expr::EqObj(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::Not(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::And(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::Or(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::Car(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::Cdr(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::SymbolToString(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::NumberToString(id) => yield (id, LocalUsedFlag::NonPhi),
                        Expr::EqNum(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::LtInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::GtInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::LeInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::GeInt(a, b) => {
                            yield (a, LocalUsedFlag::NonPhi);
                            yield (b, LocalUsedFlag::NonPhi);
                        }
                        Expr::VariadicArgs(ids) => {
                            for id in ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        Expr::VariadicArgsRef(id, _) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        Expr::VariadicArgsLength(id) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        Expr::CreateMutFuncRef(id) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        Expr::CreateEmptyMutFuncRef => {}
                        Expr::DerefMutFuncRef(id) => {
                            yield (id, LocalUsedFlag::NonPhi);
                        }
                        Expr::SetMutFuncRef(mut_func_ref_id, func_ref_id) => {
                            yield (mut_func_ref_id, LocalUsedFlag::NonPhi);
                            yield (func_ref_id, LocalUsedFlag::NonPhi);
                        }
                        Expr::EntrypointTable(mut_func_ref_ids) => {
                            for id in mut_func_ref_ids {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
                        Expr::EntrypointTableRef(_, entrypoint_table_id) => {
                            yield (entrypoint_table_id, LocalUsedFlag::NonPhi);
                        }
                        Expr::SetEntrypointTable(_, entrypoint_table_id, mut_func_ref_id) => {
                            yield (entrypoint_table_id, LocalUsedFlag::NonPhi);
                            yield (mut_func_ref_id, LocalUsedFlag::NonPhi);
                        }
                        Expr::InstantiateClosureFunc(module_id, func_id, _) => {
                            yield (module_id, LocalUsedFlag::NonPhi);
                            yield (func_id, LocalUsedFlag::NonPhi);
                        }


                        Expr::Nop
                        | Expr::InstantiateFunc(..)
                        | Expr::InstantiateBB(..)
                        | Expr::Bool(..)
                        | Expr::Int(..)
                        | Expr::Float(..)
                        | Expr::NaN
                        | Expr::String(..)
                        | Expr::Nil
                        | Expr::Char(..)
                        | Expr::CreateRef(..)
                        | Expr::FuncRef(..)
                        | Expr::GlobalGet(..) => {}
                    },
                )
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum ExprPurelity {
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

impl ExprPurelity {
    // デッドコード削除可能か
    pub fn can_dce(&self) -> bool {
        match self {
            ExprPurelity::Pure | ExprPurelity::ImpureRead => true,
            ExprPurelity::Phi | ExprPurelity::Effectful => false,
        }
    }

    // 共通部分式除去可能か
    pub fn can_cse(&self) -> bool {
        match self {
            ExprPurelity::Pure => true,
            ExprPurelity::Phi | ExprPurelity::ImpureRead | ExprPurelity::Effectful => false,
        }
    }
}

impl Expr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ Expr> {
        DisplayInFunc { value: self, meta }
    }

    pub fn purelity(&self) -> ExprPurelity {
        match self {
            Expr::Phi(..) => ExprPurelity::Phi,
            Expr::Nop
            | Expr::Bool(..)
            | Expr::Int(..)
            | Expr::Float(..)
            | Expr::NaN
            | Expr::Nil
            | Expr::Char(..)
            | Expr::FuncRef(..)
            | Expr::Move(..)
            | Expr::ToObj(..)
            | Expr::FromObj(..)
            | Expr::Closure { .. }
            | Expr::ClosureEnv(..)
            | Expr::ClosureModuleId(..)
            | Expr::ClosureFuncId(..)
            | Expr::ClosureEntrypointTable(..)
            | Expr::Is(..)
            | Expr::EqObj(..)
            | Expr::Not(..)
            | Expr::And(..)
            | Expr::Or(..)
            | Expr::EqNum(..)
            | Expr::LtInt(..)
            | Expr::GtInt(..)
            | Expr::LeInt(..)
            | Expr::GeInt(..)
            | Expr::VariadicArgs(..)
            | Expr::VariadicArgsRef(..)
            | Expr::VariadicArgsLength(..)
            | Expr::AddInt(..)
            | Expr::SubInt(..)
            | Expr::MulInt(..)
            | Expr::DivInt(..) => ExprPurelity::Pure,
            // String/Cons/Vectorなどは可変なオブジェクトを生成するので純粋ではない
            Expr::String(..)
            | Expr::StringToSymbol(..)
            | Expr::Vector(..)
            | Expr::Cons(..)
            | Expr::CreateRef(..)
            | Expr::DerefRef(..)
            | Expr::VectorLength(..)
            | Expr::VectorRef(..)
            | Expr::Car(..)
            | Expr::Cdr(..)
            | Expr::GlobalGet(..)
            | Expr::SymbolToString(..)
            | Expr::NumberToString(..)
            | Expr::CreateMutFuncRef(..)
            | Expr::CreateEmptyMutFuncRef
            | Expr::DerefMutFuncRef(..)
            | Expr::EntrypointTable(..)
            | Expr::EntrypointTableRef(..)
            | Expr::SetEntrypointTable(..) => ExprPurelity::ImpureRead,
            Expr::InstantiateFunc(..)
            | Expr::InstantiateClosureFunc(..)
            | Expr::InstantiateBB(..)
            | Expr::SetRef(..)
            | Expr::SetMutFuncRef(..)
            | Expr::Call(..)
            | Expr::CallRef(..)
            | Expr::CallClosure(..)
            | Expr::GlobalSet(..)
            | Expr::Display(..)
            | Expr::WriteChar(..)
            | Expr::VectorSet(..) => ExprPurelity::Effectful,
        }
    }

    impl_Expr_func_ids!(_mut, mut);
    impl_Expr_func_ids!(,);
    impl_Expr_local_usages!(_mut, mut);
    impl_Expr_local_usages!(,);
}

impl fmt::Display for DisplayInFunc<'_, &'_ Expr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Expr::Nop => write!(f, "nop"),
            Expr::Phi(values) => {
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
            Expr::InstantiateFunc(module_id, func_id, func_index) => {
                write!(
                    f,
                    "instantiate_func({}, {}, {})",
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta),
                    func_index
                )
            }
            Expr::InstantiateClosureFunc(module_id, func_id, func_index) => {
                write!(
                    f,
                    "instantiate_closure_func({}, {}, {})",
                    module_id.display(self.meta),
                    func_id.display(self.meta),
                    func_index
                )
            }
            Expr::InstantiateBB(module_id, func_id, func_index, bb_id, index) => {
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
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Int(i) => write!(f, "{}", i),
            Expr::Float(fl) => write!(f, "{}", fl),
            Expr::NaN => write!(f, "nan"),
            Expr::String(s) => write!(f, "{:?}", s),
            Expr::StringToSymbol(id) => write!(f, "string_to_symbol({})", id.display(self.meta)),
            Expr::Nil => write!(f, "nil"),
            Expr::Char(c) => write!(f, "{:?}", c),
            Expr::Vector(v) => {
                write!(f, "[")?;
                for (i, id) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, "]")
            }
            Expr::Cons(a, b) => {
                write!(f, "({} . {})", a.display(self.meta), b.display(self.meta))
            }
            Expr::CreateRef(typ) => write!(f, "create_ref<{}>", typ),
            Expr::DerefRef(typ, id) => {
                write!(f, "deref_ref<{}>({})", typ, id.display(self.meta))
            }
            Expr::SetRef(typ, id, value) => {
                write!(
                    f,
                    "set_ref<{}>({}, {})",
                    typ,
                    id.display(self.meta),
                    value.display(self.meta)
                )
            }
            Expr::FuncRef(id) => write!(f, "func_ref({})", id.display(self.meta.meta)),
            Expr::Call(call) => {
                write!(f, "{}", call.display(self.meta))
            }
            Expr::Closure {
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
            Expr::CallRef(call_ref) => {
                write!(f, "{}", call_ref.display(self.meta))
            }
            Expr::CallClosure(call_closure) => {
                write!(f, "{}", call_closure.display(self.meta))
            }
            Expr::Move(id) => write!(f, "move({})", id.display(self.meta)),
            Expr::ToObj(typ, id) => write!(f, "to_obj<{}>({})", typ, id.display(self.meta)),
            Expr::FromObj(typ, id) => write!(f, "from_obj<{}>({})", typ, id.display(self.meta)),
            Expr::ClosureEnv(env_types, closure, index) => {
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
            Expr::ClosureModuleId(id) => write!(f, "closure_module_id({})", id.display(self.meta)),
            Expr::ClosureFuncId(id) => write!(f, "closure_func_id({})", id.display(self.meta)),
            Expr::ClosureEntrypointTable(id) => {
                write!(f, "closure_entrypoint_table({})", id.display(self.meta))
            }
            Expr::GlobalSet(id, value) => {
                write!(
                    f,
                    "global_set({}, {})",
                    id.display(self.meta.meta),
                    value.display(self.meta)
                )
            }
            Expr::GlobalGet(id) => write!(f, "global_get({})", id.display(self.meta.meta)),
            Expr::Display(id) => write!(f, "display({})", id.display(self.meta)),
            Expr::AddInt(a, b) => {
                write!(
                    f,
                    "add_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::SubInt(a, b) => {
                write!(
                    f,
                    "sub_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::MulInt(a, b) => {
                write!(
                    f,
                    "mul_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::DivInt(a, b) => {
                write!(
                    f,
                    "div_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::WriteChar(id) => write!(f, "write_char({})", id.display(self.meta)),
            Expr::Is(typ, id) => write!(f, "is<{}>({})", typ, id.display(self.meta)),
            Expr::VectorLength(id) => write!(f, "vector_length({})", id.display(self.meta)),
            Expr::VectorRef(id, index) => {
                write!(
                    f,
                    "vector_ref({}, {})",
                    id.display(self.meta),
                    index.display(self.meta)
                )
            }
            Expr::VectorSet(id, index, value) => {
                write!(
                    f,
                    "vector_set({}, {}, {})",
                    id.display(self.meta),
                    index.display(self.meta),
                    value.display(self.meta)
                )
            }
            Expr::EqObj(a, b) => {
                write!(
                    f,
                    "eq_obj({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::Not(id) => write!(f, "not({})", id.display(self.meta)),
            Expr::And(a, b) => write!(f, "and({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Or(a, b) => write!(f, "or({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Car(id) => write!(f, "car({})", id.display(self.meta)),
            Expr::Cdr(id) => write!(f, "cdr({})", id.display(self.meta)),
            Expr::SymbolToString(id) => write!(f, "symbol_to_string({})", id.display(self.meta)),
            Expr::NumberToString(id) => write!(f, "number_to_string({})", id.display(self.meta)),
            Expr::EqNum(a, b) => write!(
                f,
                "eq_num({}, {})",
                a.display(self.meta),
                b.display(self.meta)
            ),
            Expr::LtInt(a, b) => {
                write!(
                    f,
                    "lt_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::GtInt(a, b) => {
                write!(
                    f,
                    "gt_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::LeInt(a, b) => {
                write!(
                    f,
                    "le_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::GeInt(a, b) => {
                write!(
                    f,
                    "ge_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            Expr::VariadicArgs(ids) => {
                write!(f, "variadic_args(")?;
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, ")")
            }
            Expr::VariadicArgsRef(id, index) => {
                write!(f, "variadic_args_ref({}, {})", id.display(self.meta), index)
            }
            Expr::VariadicArgsLength(id) => {
                write!(f, "variadic_args_length({})", id.display(self.meta))
            }
            Expr::CreateMutFuncRef(func_ref_id) => {
                write!(f, "create_mut_func_ref({})", func_ref_id.display(self.meta))
            }
            Expr::CreateEmptyMutFuncRef => write!(f, "create_empty_mut_func_ref"),
            Expr::DerefMutFuncRef(id) => write!(f, "deref_mut_func_ref({})", id.display(self.meta)),
            Expr::SetMutFuncRef(mut_func_ref_id, func_ref_id) => write!(
                f,
                "set_mut_func_ref({}, {})",
                mut_func_ref_id.display(self.meta),
                func_ref_id.display(self.meta)
            ),
            Expr::EntrypointTable(mut_func_ref_ids) => {
                write!(f, "entrypoint_table(")?;
                for (i, id) in mut_func_ref_ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, ")")
            }
            Expr::EntrypointTableRef(index, entrypoint_table_id) => write!(
                f,
                "entrypoint_table_ref({}, {})",
                index,
                entrypoint_table_id.display(self.meta),
            ),
            Expr::SetEntrypointTable(index, entrypoint_table_id, mut_func_ref_id) => write!(
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
pub struct ExprAssign {
    pub local: Option<LocalId>,
    pub expr: Expr,
}

macro_rules! impl_ExprAssign_local_usages {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_usages $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = (&$($mutability)? LocalId, LocalFlag)> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        for (id, used_flag) in self.expr.[<local_usages $($suffix)?>]() {
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

impl ExprAssign {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprAssign> {
        DisplayInFunc { value: self, meta }
    }

    impl_ExprAssign_local_usages!(_mut, mut);
    impl_ExprAssign_local_usages!(,);
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprAssign> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(local) = self.value.local {
            write!(f, "{}", local.display(self.meta))?;
        } else {
            write!(f, "_")?;
        }
        write!(f, " = {}", self.value.expr.display(self.meta))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub exprs: Vec<ExprAssign>,
    pub next: BasicBlockNext,
}

macro_rules! impl_BasicBlock_local_usages {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_usages $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = (&$($mutability)? LocalId, LocalFlag)> {
                from_coroutine(
                    #[coroutine]
                    move || {
                        for expr in &$($mutability)? self.exprs {
                            for usage in expr.[<local_usages $($suffix)?>]() {
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
                        for expr in &$($mutability)? self.exprs {
                            for id in expr.expr.[<func_ids $($suffix)?>]() {
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
        for expr in &self.value.exprs {
            writeln!(
                f,
                "{}{}{}",
                DISPLAY_INDENT,
                DISPLAY_INDENT,
                expr.display(self.meta)
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

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct BasicBlockId(usize);

impl BasicBlockId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, BasicBlockId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, BasicBlockId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.value.0)?;
        // TODO: add basic block name
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicBlockTerminator {
    Return(LocalId),
    TailCall(ExprCall),
    TailCallRef(ExprCallRef),
    TailCallClosure(ExprCallClosure),
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

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct ModuleId(usize);

impl ModuleId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, ModuleId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, ModuleId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module{}", self.value.0)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub globals: FxHashMap<GlobalId, Global>,
    pub funcs: TiVec<FuncId, Func>,
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
        for func in self.value.funcs.iter() {
            write!(f, "{}", func.display(self.meta))?;
        }
        Ok(())
    }
}

// TODO: ここに置くべきじゃない
#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct TypeParamId(usize);
