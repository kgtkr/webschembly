use std::{fmt, marker::PhantomData};

use derive_more::{From, Into};
use derive_where::derive_where;
use refl::{Id, refl};
use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::TiVec;

const DISPLAY_INDENT: &str = "  ";

#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
}
#[derive(Debug, Clone)]
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

pub trait LocalTypeS {}

pub struct MutCellC<T: TypeS>(T);
impl<T: TypeS> LocalTypeS for MutCellC<T> {}

pub struct TypeC<T: TypeS>(T);
impl<T: TypeS> LocalTypeS for TypeC<T> {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum LocalType {
    #[display("mut_cell({})", _0)]
    MutCell(Type),
    #[display("{}", _0)]
    Type(Type),
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

pub trait TypeS {}

pub struct BoxedC;
impl TypeS for BoxedC {}

pub struct ValC<T: ValTypeS>(T);
impl<T: ValTypeS> TypeS for ValC<T> {}

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

// Box化可能な型
// 基本的にSchemeの型に対応するがFuncRefなど例外もある
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum ValType {
    #[display("bool")]
    Bool,
    #[display("int")]
    Int,
    #[display("string")]
    String,
    #[display("symbol")]
    Symbol,
    #[display("nil")]
    Nil,
    #[display("cons")]
    Cons,
    #[display("closure")]
    Closure,
    #[display("char")]
    Char,
    #[display("vector")]
    Vector,
    #[display("func_ref")]
    FuncRef,
}

#[derive(From, Into, Hash, PartialEq, Eq)]
#[derive_where(Clone, Copy, Debug)]
pub struct LocalId<T: LocalTypeS>(usize, PhantomData<T>);

impl<T: LocalTypeS> LocalId<T> {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, LocalId<T>> {
        DisplayInFunc { value: *self, meta }
    }
}

impl<T: LocalTypeS> fmt::Display for DisplayInFunc<'_, LocalId<T>> {
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

#[derive(Debug, Clone)]
pub enum Expr<T: LocalTypeS> {
    InstantiateModule(refl::Id<TypeC<ValC<NilC>>, T>, ModuleId),
    Bool(refl::Id<TypeC<ValC<BoolC>>, T>, bool),
    Int(refl::Id<TypeC<ValC<IntC>>, T>, i64),
    String(refl::Id<TypeC<ValC<StringC>>, T>, String),
    StringToSymbol(
        refl::Id<TypeC<ValC<StringC>>, T>,
        LocalId<TypeC<ValC<StringC>>>,
    ),
    Nil(refl::Id<TypeC<ValC<NilC>>, T>),
    Char(refl::Id<TypeC<ValC<CharC>>, T>, char),
    Vector(
        refl::Id<TypeC<ValC<VectorC>>, T>,
        Vec<LocalId<TypeC<BoxedC>>>,
    ),
    Cons(
        refl::Id<TypeC<ValC<ConsC>>, T>,
        LocalId<TypeC<BoxedC>>,
        LocalId<TypeC<BoxedC>>,
    ),
    CreateMutCell(refl::Id<MutCellC<_>, T>),
    DerefMutCell(Type, LocalId),
    SetMutCell(Type, LocalId /* mutcell */, LocalId /* value */),
    FuncRef(FuncId),
    Call(bool, FuncId, Vec<LocalId>),
    Closure {
        envs: Vec<LocalId>,
        func: LocalId,
        boxed_func: LocalId,
    },
    CallRef(bool, LocalId, Vec<LocalId>, FuncType),
    Move(LocalId),
    Box(ValType, LocalId),
    Unbox(ValType, LocalId),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        LocalId,        /* closure */
        usize,          /* env index */
    ),
    ClosureFuncRef(LocalId),
    ClosureBoxedFuncRef(LocalId),
    GlobalSet(GlobalId, LocalId),
    GlobalGet(GlobalId),
    Error(LocalId),
    InitModule,
    // builtins
    Display(LocalId),
    Add(LocalId, LocalId),
    Sub(LocalId, LocalId),
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
}

impl Expr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ Expr> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ Expr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Expr::InstantiateModule(id) => {
                write!(f, "instantiate_module({})", id.display(self.meta.meta))
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
            Expr::CreateMutCell(typ) => write!(f, "create_mut_cell<{}>", typ),
            Expr::DerefMutCell(typ, id) => {
                write!(f, "deref_mut_cell<{}>({})", typ, id.display(self.meta))
            }
            Expr::SetMutCell(typ, id, value) => {
                write!(
                    f,
                    "set_mut_cell<{}>({}, {})",
                    typ,
                    id.display(self.meta),
                    value.display(self.meta)
                )
            }
            Expr::FuncRef(id) => write!(f, "func_ref({})", id.display(self.meta.meta)),
            Expr::Call(is_tail, id, args) => {
                if *is_tail {
                    write!(f, "return_")?;
                }
                write!(f, "call({})", id.display(self.meta.meta))?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        write!(f, "{}", arg.display(self.meta))?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Expr::Closure {
                envs,
                func,
                boxed_func,
            } => {
                write!(
                    f,
                    "closure(func={}, boxed_func={}",
                    func.display(self.meta),
                    boxed_func.display(self.meta)
                )?;
                for env in envs {
                    write!(f, ", {}", env.display(self.meta))?;
                }
                write!(f, ")")
            }
            Expr::CallRef(is_tail, id, args, _func_type) => {
                // TODO: func_typeを表示する
                if *is_tail {
                    write!(f, "return_")?;
                }
                write!(f, "call_ref({})", id.display(self.meta))?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        write!(f, "{}", arg.display(self.meta))?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
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
            Expr::ClosureBoxedFuncRef(id) => {
                write!(f, "closure_boxed_func_ref({})", id.display(self.meta))
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
            Expr::Error(id) => write!(f, "error({})", id.display(self.meta)),
            Expr::InitModule => {
                write!(f, "init_module")
            }
            Expr::Display(id) => write!(f, "display({})", id.display(self.meta)),
            Expr::Add(a, b) => write!(f, "add({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Sub(a, b) => write!(f, "sub({}, {})", a.display(self.meta), b.display(self.meta)),
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExprAssign {
    pub local: Option<LocalId>,
    pub expr: Expr,
}

impl ExprAssign {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprAssign> {
        DisplayInFunc { value: self, meta }
    }
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

impl BasicBlock {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ BasicBlock> {
        DisplayInFunc { value: self, meta }
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

// 閉路を作ってはいけない
#[derive(Debug, Clone, Copy)]
pub enum BasicBlockNext {
    If(LocalId, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Return,
}

impl BasicBlockNext {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, BasicBlockNext> {
        DisplayInFunc { value: *self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, BasicBlockNext> {
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
            BasicBlockNext::Return => write!(f, "return"),
        }
    }
}

impl BasicBlockNext {
    pub fn successors(&self) -> Vec<BasicBlockId> {
        match self {
            BasicBlockNext::If(_, t, f) => vec![*t, *f],
            BasicBlockNext::Jump(bb) => vec![*bb],
            BasicBlockNext::Return => vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Func {
    pub id: FuncId,
    pub locals: TiVec<LocalId, LocalType>,
    // argsとretで指定されたローカル変数の型は LocalType::Type でなければならない
    // localsの先頭何個が引数か
    pub args: usize,
    // localsのうちどれが返り値か
    pub ret: LocalId,
    pub bb_entry: BasicBlockId,
    pub bbs: TiVec<BasicBlockId, BasicBlock>,
}

impl Func {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, &'_ Func> {
        Display { value: self, meta }
    }

    pub fn arg_types(&self) -> Vec<Type> {
        (0..self.args)
            .map(|i| self.locals[LocalId::from(i)].to_type().unwrap())
            .collect()
    }

    pub fn ret_type(&self) -> Type {
        self.locals[self.ret].to_type().unwrap()
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
        for (local_id, local_type) in self.value.locals.iter_enumerated() {
            writeln!(
                f,
                "{}local {}: {}",
                DISPLAY_INDENT,
                local_id.display(self.meta.in_func(self.value.id)),
                local_type
            )?;
        }
        write!(f, "{}args: ", DISPLAY_INDENT)?;
        for i in 0..self.value.args {
            write!(
                f,
                "{}",
                LocalId::from(i).display(self.meta.in_func(self.value.id))
            )?;
            if i < self.value.args - 1 {
                write!(f, ",")?;
            }
        }
        writeln!(f)?;
        writeln!(
            f,
            "{}rets: {}",
            DISPLAY_INDENT,
            self.value.ret.display(self.meta.in_func(self.value.id))
        )?;
        writeln!(
            f,
            "{}entry: {}",
            DISPLAY_INDENT,
            self.value.bb_entry.display(self.meta)
        )?;
        for bb in self.value.bbs.iter() {
            write!(f, "{}", bb.display(self.meta.in_func(self.value.id)))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub args: Vec<Type>,
    pub ret: Type,
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
