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

    pub fn terminator(&self) -> &TerminatorInstr {
        match &self
            .instrs
            .last()
            .expect("BasicBlock has no instructions")
            .kind
        {
            InstrKind::Terminator(term) => term,
            _ => panic!("BasicBlock does not end with a Terminator instruction"),
        }
    }

    pub fn terminator_mut(&mut self) -> &mut TerminatorInstr {
        match &mut self
            .instrs
            .last_mut()
            .expect("BasicBlock has no instructions")
            .kind
        {
            InstrKind::Terminator(term) => term,
            _ => panic!("BasicBlock does not end with a Terminator instruction"),
        }
    }

    pub fn insert_instrs_before_terminator(&mut self, instrs: impl Iterator<Item = Instr>) {
        let len = self.instrs.len();
        self.instrs.splice(len - 1..len - 1, instrs);
    }
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

    pub fn extend_entry_bb(&mut self, f: impl FnOnce(&mut Func, TerminatorInstr) -> BasicBlockId) {
        let prev_entry_bb_id = self.bb_entry;
        let new_entry_bb_id = f(self, TerminatorInstr::Jump(prev_entry_bb_id));
        self.bb_entry = new_entry_bb_id;
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

impl Default for Module {
    fn default() -> Self {
        Self::new()
    }
}

impl Module {
    pub fn new() -> Self {
        let mut funcs: VecMap<FuncId, Func> = VecMap::new();
        let entry = funcs.push_with(|id| {
            let mut locals = VecMap::new();
            let ret_local_id = locals.push_with(|local_id| Local {
                id: local_id,
                typ: ValType::Nil.into(),
            });

            let mut bbs = VecMap::new();
            let bb_entry = bbs.push_with(|bb_id| BasicBlock {
                id: bb_id,
                instrs: vec![
                    Instr {
                        local: Some(ret_local_id),
                        kind: InstrKind::Nil,
                    },
                    Instr {
                        local: None,
                        kind: InstrKind::Terminator(TerminatorInstr::Exit(ExitInstr::Return(
                            ret_local_id,
                        ))),
                    },
                ],
            });

            Func {
                id,
                locals,
                args: Vec::new(),
                ret_type: ValType::Nil.into(),
                bbs,
                bb_entry,
            }
        });

        Self {
            globals: FxHashMap::default(),
            funcs,
            entry,
            meta: Meta {
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }

    pub fn display(&self) -> Display<'_, &Module> {
        Display {
            value: self,
            meta: &self.meta,
        }
    }

    pub fn extend_entry_func(
        &mut self,
        f: impl FnOnce(&mut Func, TerminatorInstr) -> BasicBlockId,
    ) {
        let entry_func = &mut self.funcs[self.entry];

        entry_func.extend_entry_bb(f);
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
