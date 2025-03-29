use derive_more::{From, Into};
use typed_index_collections::TiVec;

use crate::ast;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum LocalType {
    MutCell, // 中身はBoxed固定
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
            LocalType::MutCell => Type::Boxed,
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
}

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    String(String),
    StringToSymbol(usize),
    Nil,
    Char(char),
    Cons(usize, usize),
    CreateMutCell,
    DerefMutCell(usize),
    SetMutCell(usize /* mutcell */, usize /* value */),
    Closure(Vec<usize>, usize),
    CallClosure(bool, usize, Vec<usize>),
    Move(usize),
    Box(ValType, usize),
    Unbox(ValType, usize),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        usize,          /* closure */
        usize,          /* env index */
    ),
    GlobalSet(usize, usize),
    GlobalGet(usize),
    // Builtin = BuiltinClosure + CallClosureだが後から最適化するのは大変なので一旦分けておく
    Builtin(ast::Builtin, Vec<usize>), // TODO: astを参照するべきではない
    GetBuiltin(ast::Builtin),
    SetBuiltin(ast::Builtin, usize),
    Error(usize),
    InitGlobals(usize),  // global count
    InitBuiltins(usize), // builtin count
}

#[derive(Debug, Clone)]
pub struct ExprAssign {
    pub local: Option<usize>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub exprs: Vec<ExprAssign>,
    pub next: BasicBlockNext,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct BasicBlockId(usize);

// 閉路を作ってはいけない
#[derive(Debug, Clone, Copy)]
pub enum BasicBlockNext {
    If(usize, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Return,
}

#[derive(Debug, Clone)]
pub struct Func {
    pub locals: Vec<LocalType>,
    // localsの先頭何個が引数か
    pub args: usize,
    // localsのうちどれが返り値か
    pub rets: Vec<usize>,
    pub bb_entry: BasicBlockId,
    pub bbs: TiVec<BasicBlockId, BasicBlock>,
}

impl Func {
    pub fn arg_types(&self) -> Vec<Type> {
        (0..self.args).map(|i| self.locals[i].to_type()).collect()
    }

    pub fn ret_types(&self) -> Vec<Type> {
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
    pub args: Vec<Type>,
    pub rets: Vec<Type>,
}

#[derive(Debug, Clone)]
pub struct Ir {
    pub funcs: Vec<Func>,
    pub entry: usize,
}
