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
