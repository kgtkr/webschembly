use std::{fmt, iter::from_coroutine};

use derive_more::{From, Into};
use rustc_hash::{FxHashMap, FxHashSet};
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
    fn in_func(&self, func_id: FuncId) -> MetaInFunc {
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
    Ref(Type), // TODO: Ref<Boxed>固定で良いかも？
    #[display("{}", _0)]
    Type(Type),
    #[display("args")]
    Args,
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
    #[display("boxed")]
    Boxed,
    #[display("{}", _0)]
    Val(ValType),
}

impl From<ValType> for Type {
    fn from(typ: ValType) -> Self {
        Self::Val(typ)
    }
}

// Box化可能な型
// 基本的にSchemeの型に対応するがFuncRefなど例外もある
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display, EnumIter)]
#[repr(i32)]
pub enum ValType {
    #[display("nil")]
    Nil = 1,
    #[display("bool")]
    Bool = 2,
    #[display("char")]
    Char = 3,
    #[display("int")]
    Int = 4,
    #[display("string")]
    String = 5,
    #[display("symbol")]
    Symbol = 6,
    #[display("cons")]
    Cons = 7,
    #[display("vector")]
    Vector = 8,
    #[display("func_ref")]
    FuncRef = 9,
    #[display("closure")]
    Closure = 10,
}

impl ValType {
    pub fn tag(&self) -> i32 {
        *self as i32
    }
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
    Used,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
            "call_ref({}:{})",
            self.value.func.display(self.meta),
            self.value.func_type
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Nop,
    InstantiateFunc(ModuleId, FuncId),
    InstantiateBB(ModuleId, FuncId, BasicBlockId, LocalId /* index */),
    Bool(bool),
    Int(i64),
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
        func: LocalId,
    },
    Move(LocalId),
    Box(ValType, LocalId),
    Unbox(ValType, LocalId),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        LocalId,        /* closure */
        usize,          /* env index */
    ),
    ClosureFuncRef(LocalId),
    GlobalSet(GlobalId, LocalId),
    GlobalGet(GlobalId),
    Error(LocalId),
    InitModule,
    // builtins
    Display(LocalId),
    Add(LocalId, LocalId),
    Sub(LocalId, LocalId),
    Mul(LocalId, LocalId),
    Div(LocalId, LocalId),
    WriteChar(LocalId),
    IsPair(LocalId),
    IsSymbol(LocalId),
    IsString(LocalId),
    IsNumber(LocalId),
    IsBoolean(LocalId),
    IsProcedure(LocalId),
    IsChar(LocalId),
    IsVector(LocalId),
    VectorLength(LocalId),
    VectorRef(LocalId, LocalId),
    VectorSet(LocalId, LocalId, LocalId),
    Eq(LocalId, LocalId),
    Not(LocalId),
    Car(LocalId),
    Cdr(LocalId),
    SymbolToString(LocalId),
    NumberToString(LocalId),
    EqNum(LocalId, LocalId),
    Lt(LocalId, LocalId),
    Gt(LocalId, LocalId),
    Le(LocalId, LocalId),
    Ge(LocalId, LocalId),
    Args(Vec<LocalId>),
    ArgsRef(LocalId, usize),
    // ArgsVariadic(Vec<LocalId>, LocalId<Vector>)
    // ArgsRest(LocalId, usize) -> Vector
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

macro_rules! impl_Expr_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        Expr::StringToSymbol(id) => yield id,
                        Expr::Vector(ids) => {
                            for id in ids {
                                yield id;
                            }
                        }
                        Expr::Cons(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::DerefRef(_, id) => yield id,
                        Expr::SetRef(_, ref_id, value_id) => {
                            yield ref_id;
                            yield value_id;
                        }
                        Expr::Call(call) => {
                            for id in call.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        Expr::Closure {
                            envs,
                            func,
                        } => {
                            for env in envs {
                                yield env;
                            }
                            yield func;
                        }
                        Expr::CallRef(call_ref) => {
                            for id in call_ref.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        Expr::Move(id) => yield id,
                        Expr::Box(_, id) => yield id,
                        Expr::Unbox(_, id) => yield id,
                        Expr::ClosureEnv(_, closure, _) => yield closure,
                        Expr::ClosureFuncRef(id) => yield id,
                        Expr::GlobalSet(_, value) => yield value,
                        Expr::Error(id) => yield id,
                        Expr::Display(id) => yield id,
                        Expr::Add(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Sub(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Mul(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Div(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::WriteChar(id) => yield id,
                        Expr::IsPair(id) => yield id,
                        Expr::IsSymbol(id) => yield id,
                        Expr::IsString(id) => yield id,
                        Expr::IsNumber(id) => yield id,
                        Expr::IsBoolean(id) => yield id,
                        Expr::IsProcedure(id) => yield id,
                        Expr::IsChar(id) => yield id,
                        Expr::IsVector(id) => yield id,
                        Expr::VectorLength(id) => yield id,
                        Expr::VectorRef(vec_id, index_id) => {
                            yield vec_id;
                            yield index_id;
                        }
                        Expr::VectorSet(vec_id, index_id, value_id) => {
                            yield vec_id;
                            yield index_id;
                            yield value_id;
                        }
                        Expr::Eq(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Not(id) => yield id,
                        Expr::Car(id) => yield id,
                        Expr::Cdr(id) => yield id,
                        Expr::SymbolToString(id) => yield id,
                        Expr::NumberToString(id) => yield id,
                        Expr::EqNum(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Lt(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Gt(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Le(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Ge(a, b) => {
                            yield a;
                            yield b;
                        }
                        Expr::Args(ids) => {
                            for id in ids {
                                yield id;
                            }
                        }
                        Expr::ArgsRef(id, _) => {
                            yield id;
                        }

                        Expr::Nop
                        | Expr::InstantiateFunc(..)
                        | Expr::InstantiateBB(..)
                        | Expr::Bool(..)
                        | Expr::Int(..)
                        | Expr::String(..)
                        | Expr::Nil
                        | Expr::Char(..)
                        | Expr::CreateRef(..)
                        | Expr::FuncRef(..)
                        | Expr::GlobalGet(..)
                        | Expr::InitModule => {}
                    },
                )
            }
        }
    };
}

impl Expr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ Expr> {
        DisplayInFunc { value: self, meta }
    }

    // 結果が使われていなければ削除しても良い命令か？
    pub fn is_effectful(&self) -> bool {
        match self {
            Expr::Nop
            | Expr::Bool(..)
            | Expr::Int(..)
            | Expr::String(..)
            | Expr::StringToSymbol(..)
            | Expr::Nil
            | Expr::Char(..)
            | Expr::Vector(..)
            | Expr::Cons(..)
            | Expr::FuncRef(..)
            | Expr::Move(..)
            | Expr::Box(..)
            // 型エラーが起きる可能性があるので厳密には副作用ありだが一旦
            | Expr::Unbox(..)
            | Expr::Closure { .. }
            | Expr::ClosureEnv(..)
            | Expr::ClosureFuncRef(..)
            | Expr::GlobalGet(..)
            | Expr::IsPair(..)
            | Expr::IsSymbol(..)
            | Expr::IsString(..)
            | Expr::IsNumber(..)
            | Expr::IsBoolean(..)
            | Expr::IsProcedure(..)
            | Expr::IsChar(..)
            | Expr::IsVector(..)
            | Expr::VectorLength(..)
            | Expr::VectorRef(..)
            | Expr::Eq(..)
            | Expr::Not(..)
            | Expr::Car(..)
            | Expr::Cdr(..)
            | Expr::SymbolToString(..)
            | Expr::NumberToString(..)
            | Expr::EqNum(..)
            | Expr::Lt(..)
            | Expr::Gt(..)
            | Expr::Le(..)
            | Expr::Ge(..)
            | Expr::Args(..)
            | Expr::ArgsRef(..)
            | Expr::CreateRef(..)
            | Expr::DerefRef(..)
            | Expr::Add(..)
            | Expr::Sub(..)
            | Expr::Mul(..)
            | Expr::Div(..) => false,

            Expr::InstantiateFunc(..)
            | Expr::InstantiateBB(..)
            | Expr::SetRef(..)
            | Expr::Call(..)
            | Expr::CallRef(..)
            | Expr::GlobalSet(..)
            | Expr::Error(..)
            | Expr::InitModule
            | Expr::Display(..)
            | Expr::WriteChar(..)
            | Expr::VectorSet(..) => true,
        }
    }

    impl_Expr_func_ids!(_mut, mut);
    impl_Expr_func_ids!(,);
    impl_Expr_local_ids!(_mut, mut);
    impl_Expr_local_ids!(,);
}

impl fmt::Display for DisplayInFunc<'_, &'_ Expr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Expr::Nop => write!(f, "nop"),
            Expr::InstantiateFunc(module_id, func_id) => {
                write!(
                    f,
                    "instantiate_func({}, {})",
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta)
                )
            }
            Expr::InstantiateBB(module_id, func_id, bb_id, index) => {
                write!(
                    f,
                    "instantiate_bb({}, {}, {}, {})",
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta),
                    bb_id.display(self.meta.meta),
                    index.display(self.meta),
                )
            }
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Int(i) => write!(f, "{}", i),
            Expr::String(s) => write!(f, "\"{}\"", s),
            Expr::StringToSymbol(id) => write!(f, "string_to_symbol({})", id.display(self.meta)),
            Expr::Nil => write!(f, "nil"),
            Expr::Char(c) => write!(f, "'{}'", c),
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
            Expr::Closure { envs, func } => {
                write!(f, "closure(func={}", func.display(self.meta),)?;
                for env in envs {
                    write!(f, ", {}", env.display(self.meta))?;
                }
                write!(f, ")")
            }
            Expr::CallRef(call_ref) => {
                write!(f, "{}", call_ref.display(self.meta))
            }
            Expr::Move(id) => write!(f, "move({})", id.display(self.meta)),
            Expr::Box(typ, id) => write!(f, "box<{}>({})", typ, id.display(self.meta)),
            Expr::Unbox(typ, id) => write!(f, "unbox<{}>({})", typ, id.display(self.meta)),
            Expr::ClosureEnv(env_types, closure, index) => {
                write!(f, "closure_env<")?;
                for (i, typ) in env_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", typ)?;
                }
                write!(f, ">({}, {}]", closure.display(self.meta), index)?;
                Ok(())
            }
            Expr::ClosureFuncRef(id) => write!(f, "closure_func_ref({})", id.display(self.meta)),
            Expr::GlobalSet(id, value) => {
                write!(
                    f,
                    "global_set({}, {})",
                    id.display(self.meta.meta),
                    value.display(self.meta)
                )
            }
            Expr::GlobalGet(id) => write!(f, "global_get({})", id.display(self.meta.meta)),
            Expr::Error(id) => write!(f, "error({})", id.display(self.meta)),
            Expr::InitModule => {
                write!(f, "init_module")
            }
            Expr::Display(id) => write!(f, "display({})", id.display(self.meta)),
            Expr::Add(a, b) => write!(f, "add({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Sub(a, b) => write!(f, "sub({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Mul(a, b) => write!(f, "mul({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Div(a, b) => write!(f, "div({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::WriteChar(id) => write!(f, "write_char({})", id.display(self.meta)),
            Expr::IsPair(id) => write!(f, "is_pair({})", id.display(self.meta)),
            Expr::IsSymbol(id) => write!(f, "is_symbol({})", id.display(self.meta)),
            Expr::IsString(id) => write!(f, "is_string({})", id.display(self.meta)),
            Expr::IsNumber(id) => write!(f, "is_number({})", id.display(self.meta)),
            Expr::IsBoolean(id) => write!(f, "is_boolean({})", id.display(self.meta)),
            Expr::IsProcedure(id) => write!(f, "is_procedure({})", id.display(self.meta)),
            Expr::IsChar(id) => write!(f, "is_char({})", id.display(self.meta)),
            Expr::IsVector(id) => write!(f, "is_vector({})", id.display(self.meta)),
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
            Expr::Eq(a, b) => write!(f, "eq({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Not(id) => write!(f, "not({})", id.display(self.meta)),
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
            Expr::Lt(a, b) => write!(f, "lt({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Gt(a, b) => write!(f, "gt({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Le(a, b) => write!(f, "le({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Ge(a, b) => write!(f, "ge({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Args(ids) => {
                write!(f, "args(")?;
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, ")")
            }
            Expr::ArgsRef(id, index) => write!(f, "args_ref({}, {})", id.display(self.meta), index),
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
                        for id in self.expr.[<local_ids $($suffix)?>]() {
                            yield (id, LocalFlag::Used);
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

#[derive(Debug, Clone)]
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
                            yield (id, LocalFlag::Used);
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
                        | BasicBlockTerminator::TailCallRef(_) => {}
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
        }
    }
}

// 閉路を作ってはいけない
#[derive(Debug, Clone)]
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
    pub globals: FxHashSet<GlobalId>,
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
        write!(f, "globals:")?;
        for (i, global) in self.value.globals.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, " {}", global.display(self.meta))?;
        }
        writeln!(f)?;
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
