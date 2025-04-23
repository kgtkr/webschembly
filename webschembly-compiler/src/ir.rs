use derive_more::{From, Into};
use rustc_hash::FxHashMap;
use typed_index_collections::TiVec;

use crate::ast;

#[derive(Debug, Clone)]
pub struct WithMeta<T> {
    pub value: T,
    pub local_metas: FxHashMap<ast::LocalVarId, ast::VarMeta>,
    pub global_metas: FxHashMap<ast::GlobalVarId, ast::VarMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum LocalType {
    MutCell(Type),
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
    pub fn to_type(&self) -> Type {
        match self {
            LocalType::MutCell(inner) => *inner,
            LocalType::Type(typ) => *typ,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Type {
    Boxed,
    Val(ValType),
}

impl From<ValType> for Type {
    fn from(typ: ValType) -> Self {
        Self::Val(typ)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum ValType {
    Bool,
    Int,
    String,
    Symbol,
    Nil,
    Cons,
    Closure,
    Char,
    Vector,
    FuncRef,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct LocalId(usize);

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct GlobalId(usize);

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct FuncId(usize);

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    String(String),
    StringToSymbol(LocalId),
    Nil,
    Char(char),
    Vector(Vec<LocalId>),
    Cons(LocalId, LocalId),
    CreateMutCell(Type),
    DerefMutCell(Type, LocalId),
    SetMutCell(Type, LocalId /* mutcell */, LocalId /* value */),
    FuncRef(FuncId),
    Call(bool, FuncId, Vec<LocalId>),
    Closure {
        envs: Vec<LocalId>,
        func: LocalId,
        boxed_func: LocalId,
    },
    CallRef(bool, LocalId, Vec<LocalId>),
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
    InitGlobals(usize), // global count
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

#[derive(Debug, Clone)]
pub struct ExprAssign {
    pub local: Option<LocalId>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub exprs: Vec<ExprAssign>,
    pub next: BasicBlockNext,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct BasicBlockId(usize);

// 閉路を作ってはいけない
#[derive(Debug, Clone, Copy)]
pub enum BasicBlockNext {
    If(LocalId, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Return,
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
    // localsの先頭何個が引数か
    pub args: usize,
    // localsのうちどれが返り値か
    pub rets: Vec<LocalId>,
    pub bb_entry: BasicBlockId,
    pub bbs: TiVec<BasicBlockId, BasicBlock>,
}

impl Func {
    pub fn arg_types(&self) -> TiVec<FuncId, Type> {
        (0..self.args)
            .map(|i| self.locals[LocalId::from(i)].to_type())
            .collect()
    }

    pub fn ret_types(&self) -> TiVec<FuncId, Type> {
        self.rets
            .iter()
            .map(|&ret| self.locals[ret].to_type())
            .collect()
    }

    pub fn func_type(&self) -> FuncType {
        FuncType {
            args: self.arg_types(),
            rets: self.ret_types(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub args: TiVec<FuncId, Type>,
    pub rets: TiVec<FuncId, Type>,
}

#[derive(Debug, Clone)]
pub struct Ir {
    pub funcs: TiVec<FuncId, Func>,
    pub entry: FuncId,
}
