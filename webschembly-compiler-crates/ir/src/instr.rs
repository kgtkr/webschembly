use std::fmt;
use std::iter::from_coroutine;

use super::display::*;
use super::id::*;
use super::local_flag::*;
use super::meta::*;
use super::typ::*;
use ordered_float::NotNan;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhiIncomingValue {
    pub bb: BasicBlockId,
    pub local: LocalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchKind {
    Then,
    Else,
}

impl fmt::Display for BranchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchKind::Then => write!(f, "then"),
            BranchKind::Else => write!(f, "else"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExitInstr {
    Return(LocalId),
    TailCall(InstrCall),
    TailCallRef(InstrCallRef),
    TailCallClosure(InstrCallClosure),
    Error(LocalId),
}
macro_rules! impl_ExitInstr_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        ExitInstr::Return(local) => yield local,
                        ExitInstr::TailCall(call) => {
                            for id in call.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        ExitInstr::TailCallRef(call_ref) => {
                            for id in call_ref.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        ExitInstr::TailCallClosure(call_closure) => {
                            for id in call_closure.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                        ExitInstr::Error(local) => yield local,
                    },
                )
            }
        }
    };
}

macro_rules! impl_ExitInstr_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        ExitInstr::Return(_)
                        | ExitInstr::TailCallRef(_)
                        | ExitInstr::TailCallClosure(_)
                        | ExitInstr::Error(_) => {}
                        ExitInstr::TailCall(call) => {
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

impl ExitInstr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &ExitInstr> {
        DisplayInFunc { value: self, meta }
    }

    impl_ExitInstr_local_ids!(_mut, mut);
    impl_ExitInstr_local_ids!(,);

    impl_ExitInstr_func_ids!(_mut, mut);
    impl_ExitInstr_func_ids!(,);
}

impl fmt::Display for DisplayInFunc<'_, &ExitInstr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            ExitInstr::Return(local) => write!(f, "return {}", local.display(self.meta)),
            ExitInstr::TailCall(call) => {
                write!(f, "tail_call {}", call.display(self.meta))
            }
            ExitInstr::TailCallRef(call_ref) => {
                write!(f, "tail_call_ref {}", call_ref.display(self.meta))
            }
            ExitInstr::TailCallClosure(call_closure) => {
                write!(f, "tail_call_closure {}", call_closure.display(self.meta))
            }
            ExitInstr::Error(local) => write!(f, "error {}", local.display(self.meta)),
        }
    }
}

// 閉路を作ってはいけない
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TerminatorInstr {
    If(LocalId, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Exit(ExitInstr),
}

macro_rules! impl_TerminatorInstr_local_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<local_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? LocalId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        TerminatorInstr::If(cond, _, _) => yield cond,
                        TerminatorInstr::Jump(_) => {}
                        TerminatorInstr::Exit(exit) => {
                            for id in exit.[<local_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                    },
                )
            }
        }
    };
}

macro_rules! impl_TerminatorInstr_func_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<func_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? FuncId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        TerminatorInstr::If(_, _, _)
                        | TerminatorInstr::Jump(_) => {}
                        TerminatorInstr::Exit(exit) => {
                            for id in exit.[<func_ids $($suffix)?>]() {
                                yield id;
                            }
                        }
                    },
                )
            }
        }
    };
}

macro_rules! impl_TerminatorInstr_bb_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<bb_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? BasicBlockId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        TerminatorInstr::If(_, bb1, bb2) => {
                            yield bb1;
                            yield bb2;
                        }
                        TerminatorInstr::Jump(bb) => yield bb,
                        TerminatorInstr::Exit(_) => {}
                    },
                )
            }
        }
    };
}

impl TerminatorInstr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &TerminatorInstr> {
        DisplayInFunc { value: self, meta }
    }

    impl_TerminatorInstr_local_ids!(_mut, mut);
    impl_TerminatorInstr_local_ids!(,);

    impl_TerminatorInstr_func_ids!(_mut, mut);
    impl_TerminatorInstr_func_ids!(,);

    impl_TerminatorInstr_bb_ids!(_mut, mut);
    impl_TerminatorInstr_bb_ids!(,);

    // TODO: bb_idsと同じ内容なので移行する
    pub fn successors(&self) -> impl Iterator<Item = BasicBlockId> {
        self.bb_ids().copied()
    }
}

impl fmt::Display for DisplayInFunc<'_, &TerminatorInstr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            TerminatorInstr::If(cond, bb1, bb2) => {
                write!(
                    f,
                    "if {} then {} else {}",
                    cond.display(self.meta),
                    bb1.display(self.meta.meta),
                    bb2.display(self.meta.meta)
                )
            }
            TerminatorInstr::Jump(bb) => write!(f, "jump {}", bb.display(self.meta.meta)),
            TerminatorInstr::Exit(exit) => {
                write!(f, "{}", exit.display(self.meta))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstrKind {
    Nop, // 左辺はNoneでなければならない
    Phi {
        incomings: Vec<PhiIncomingValue>,
        non_exhaustive: bool,
    }, // BBの先頭にのみ連続して出現可能(Nopが間に入るのは可)。non_exhaustive=trueの時incomings.length=1でもコピー伝播などの最適化を行ってはならない(inline化のためのフラグ)
    Terminator(TerminatorInstr), // 左辺はNoneでなければならない。また、BasicBlockの最後にのみ出現可能
    InstantiateFunc(JitModuleId, JitFuncId, usize),
    InstantiateClosureFunc(LocalId, LocalId, usize), // InstantiateFuncのModuleId/FuncIdを動的に指定する版
    // TODO: InstantiateBBなどはFooId型ではなくusize型を受け取るべき
    // 理由: 副作用命令であり、BasicBlockIdの一括置換などで同時に置き換えると意味が変わってしまうため
    InstantiateBB(JitModuleId, JitFuncId, usize, JitBasicBlockId, usize),
    IncrementBranchCounter(
        JitModuleId,
        JitFuncId,
        usize,
        JitBasicBlockId,
        BranchKind,
        // 以下は呼び出し元のBBとindex
        JitBasicBlockId,
        usize,
    ),
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
        envs: Vec<Option<LocalId>>, // None: letrecなどで使われる未初期化値
        env_types: Vec<LocalType>,
        module_id: JitModuleId,
        func_id: JitFuncId,
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
    // 未初期化値で初期化されたEnvに対して一度のみ値を設定できる
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
    QuotientInt(LocalId, LocalId),
    RemainderInt(LocalId, LocalId),
    ModuloInt(LocalId, LocalId),
    DivFloat(LocalId, LocalId),
    WriteChar(LocalId),
    Is(ValType, LocalId),
    VectorLength(LocalId),
    VectorRef(LocalId, LocalId),
    VectorSet(LocalId, LocalId, LocalId),
    MakeVector(LocalId),
    UVectorLength(UVectorKind, LocalId),
    UVectorRef(UVectorKind, LocalId, LocalId),
    UVectorSet(UVectorKind, LocalId, LocalId, LocalId),
    EqObj(LocalId, LocalId),
    Not(LocalId),
    And(LocalId, LocalId),
    Or(LocalId, LocalId),
    Car(LocalId),
    Cdr(LocalId),
    SetCar(LocalId, LocalId),
    SetCdr(LocalId, LocalId),
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
    // VariadicArgs -> Cons | Nil
    VariadicArgsRest(LocalId, usize),
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
                        InstrKind::Phi { incomings, .. } => {
                            for value in incomings.[<iter $($suffix)?>]() {
                                yield (&$($mutability)? value.local, LocalUsedFlag::Phi(value.bb));
                            }
                        }
                        InstrKind::Terminator(terminator) => {
                            for id in terminator.[<local_ids $($suffix)?>]() {
                                yield (id, LocalUsedFlag::NonPhi);
                            }
                        }
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
                        InstrKind::MakeVector(id) => yield (id, LocalUsedFlag::NonPhi),
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
                            env_types: _,
                            module_id: _,
                            func_id: _,
                            entrypoint_table,
                        } => {
                            for env in envs {
                                if let Some(env) = env {
                                    yield (env, LocalUsedFlag::NonPhi);
                                }
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
                        | InstrKind::QuotientInt(a, b)
                        | InstrKind::RemainderInt(a, b)
                        | InstrKind::ModuloInt(a, b)
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
                        InstrKind::SetCar(cons_id, value_id) => {
                            yield (cons_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
                        InstrKind::SetCdr(cons_id, value_id) => {
                            yield (cons_id, LocalUsedFlag::NonPhi);
                            yield (value_id, LocalUsedFlag::NonPhi);
                        }
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
                        InstrKind::VariadicArgsRest(id, _) => {
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
                        | InstrKind::IncrementBranchCounter(..)
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

macro_rules! impl_InstrKind_bb_ids {
    ($($suffix: ident)?,$($mutability: tt)?) => {
        paste::paste! {
            pub fn [<bb_ids $($suffix)?>](&$($mutability)? self) -> impl Iterator<Item = &$($mutability)? BasicBlockId> {
                from_coroutine(
                    #[coroutine]
                    move || match self {
                        InstrKind::Phi { incomings, .. } => {
                            for value in incomings.[<iter $($suffix)?>]() {
                                yield &$($mutability)? value.bb;
                            }
                        }
                        InstrKind::Terminator(terminator) => {
                            for bb_id in terminator.[<bb_ids $($suffix)?>]() {
                                yield bb_id;
                            }
                        }
                        _ => {}
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
            InstrKindPurelity::Pure | InstrKindPurelity::Phi | InstrKindPurelity::ImpureRead => {
                true
            }
            InstrKindPurelity::Effectful => false,
        }
    }

    // 共通部分式除去可能か
    pub fn can_cse(&self) -> bool {
        match self {
            InstrKindPurelity::Pure => true,
            InstrKindPurelity::Phi // phiをcse対象にするとphiの間にmoveが入る可能性があるため不可
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
            InstrKind::Phi { .. } => InstrKindPurelity::Phi,
            InstrKind::Nop
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
            | InstrKind::QuotientInt(..)
            | InstrKind::RemainderInt(..)
            | InstrKind::ModuloInt(..)
            | InstrKind::DivFloat(..) => InstrKindPurelity::Pure,
            // String/Cons/Vectorなどは可変なオブジェクトを生成するので純粋ではない
            InstrKind::String(..)
            | InstrKind::StringToSymbol(..)
            | InstrKind::Vector(..)
            | InstrKind::UVector(..)
            | InstrKind::MakeUVector(..)
            | InstrKind::MakeVector(..)
            | InstrKind::Cons(..)
            | InstrKind::CreateRef(..)
            | InstrKind::DerefRef(..)
            | InstrKind::VectorLength(..)
            | InstrKind::VectorRef(..)
            | InstrKind::UVectorLength(..) // vector/uvectorの長さは不変なのでpureでいいのでは？
            | InstrKind::UVectorRef(..)
            | InstrKind::Car(..)
            | InstrKind::Cdr(..)
            | InstrKind::VariadicArgsRest(..)
            | InstrKind::GlobalGet(..)
            | InstrKind::SymbolToString(..)
            | InstrKind::IntToString(..)
            | InstrKind::FloatToString(..)
            | InstrKind::CreateMutFuncRef(..)
            | InstrKind::CreateEmptyMutFuncRef
            | InstrKind::DerefMutFuncRef(..)
            | InstrKind::EntrypointTable(..)
            | InstrKind::EntrypointTableRef(..)
            // closureの環境は可変である
            | InstrKind::Closure { .. }
            | InstrKind::ClosureEnv(..) => InstrKindPurelity::ImpureRead,
            InstrKind::Terminator(..)
            | InstrKind::InstantiateFunc(..)
            | InstrKind::InstantiateClosureFunc(..)
            | InstrKind::InstantiateBB(..)
            | InstrKind::IncrementBranchCounter(..)
            | InstrKind::SetRef(..)
            | InstrKind::SetMutFuncRef(..)
            | InstrKind::Call(..)
            | InstrKind::CallRef(..)
            | InstrKind::CallClosure(..)
            | InstrKind::GlobalSet(..)
            | InstrKind::Display(..)
            | InstrKind::WriteChar(..)
            | InstrKind::SetEntrypointTable(..)
            | InstrKind::VectorSet(..)
            | InstrKind::UVectorSet(..)
            | InstrKind::ClosureSetEnv(..)
            | InstrKind::SetCar(..)
            | InstrKind::SetCdr(..)
             => InstrKindPurelity::Effectful,
        }
    }

    impl_InstrKind_func_ids!(_mut, mut);
    impl_InstrKind_func_ids!(,);
    impl_InstrKind_local_usages!(_mut, mut);
    impl_InstrKind_local_usages!(,);
    impl_InstrKind_bb_ids!(_mut, mut);
    impl_InstrKind_bb_ids!(,);
}

impl fmt::Display for DisplayInFunc<'_, &'_ InstrKind> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            InstrKind::Nop => write!(f, "nop"),
            InstrKind::Phi {
                incomings,
                non_exhaustive,
            } => {
                write!(f, "phi(")?;
                for (i, value) in incomings.iter().enumerate() {
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
                if *non_exhaustive {
                    if !incomings.is_empty() {
                        write!(f, ", ")?;
                    }
                    write!(f, "...")?;
                }
                write!(f, ")")
            }
            InstrKind::Terminator(terminator) => {
                write!(f, "{}", terminator.display(self.meta))
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
            InstrKind::IncrementBranchCounter(
                module_id,
                func_id,
                func_index,
                bb_id,
                branch_kind,
                source_bb_id,
                source_index,
            ) => {
                write!(
                    f,
                    "increment_branch_counter({}, {}, {}, {}, {}, {}, {})",
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta),
                    func_index,
                    bb_id.display(self.meta.meta),
                    branch_kind,
                    source_bb_id.display(self.meta.meta),
                    source_index,
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
            InstrKind::MakeVector(id) => {
                write!(f, "make_vector({})", id.display(self.meta))
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
                env_types,
                module_id,
                func_id,
                entrypoint_table,
            } => {
                write!(f, "closure<")?;
                for (i, typ) in env_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", typ)?;
                }
                write!(
                    f,
                    ">(entrypoint_table={}, module_id={}, func_id={}",
                    entrypoint_table.display(self.meta),
                    module_id.display(self.meta.meta),
                    func_id.display(self.meta.meta)
                )?;
                for env in envs {
                    write!(f, ", ")?;
                    match env {
                        Some(env) => write!(f, "{}", env.display(self.meta))?,
                        None => write!(f, "(uninitialized)")?,
                    }
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
            InstrKind::QuotientInt(a, b) => {
                write!(
                    f,
                    "quotient_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::RemainderInt(a, b) => {
                write!(
                    f,
                    "remainder_int({}, {})",
                    a.display(self.meta),
                    b.display(self.meta)
                )
            }
            InstrKind::ModuloInt(a, b) => {
                write!(
                    f,
                    "modulo_int({}, {})",
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
            InstrKind::SetCar(cons_id, value_id) => {
                write!(
                    f,
                    "set_car({}, {})",
                    cons_id.display(self.meta),
                    value_id.display(self.meta)
                )
            }
            InstrKind::SetCdr(cons_id, value_id) => {
                write!(
                    f,
                    "set_cdr({}, {})",
                    cons_id.display(self.meta),
                    value_id.display(self.meta)
                )
            }
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
            InstrKind::VariadicArgsRest(id, start_index) => {
                write!(
                    f,
                    "variadic_args_rest({}, {})",
                    id.display(self.meta),
                    start_index
                )
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
