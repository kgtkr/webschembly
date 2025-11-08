use std::fmt;
use std::iter::from_coroutine;

use super::display::*;
use super::id::*;
use super::instr::*;
use super::local_flag::*;
use super::meta::*;
use super::typ::*;
use rustc_hash::FxHashMap;
use vec_map::{HasId, VecMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub instrs: Vec<Instr>,
    pub next: TerminatorInstr,
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
pub enum TerminatorInstr {
    If(LocalId, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Exit(BasicBlockTerminator),
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
