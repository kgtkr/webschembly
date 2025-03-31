use derive_more::{From, Into};
use strum_macros::{EnumIter, EnumString, FromRepr, IntoStaticStr};
use typed_index_collections::{TiVec, ti_vec};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, FromRepr, EnumString, IntoStaticStr,
)]
pub enum Builtin {
    #[strum(serialize = "display")]
    Display, // TODO: 将来的には組み込み関数ではなくしたい
    #[strum(serialize = "+")]
    Add,
    #[strum(serialize = "-")]
    Sub,
    #[strum(serialize = "write-char")]
    WriteChar,
    #[strum(serialize = "pair?")]
    IsPair,
    #[strum(serialize = "symbol?")]
    IsSymbol,
    #[strum(serialize = "string?")]
    IsString,
    #[strum(serialize = "number?")]
    IsNumber,
    #[strum(serialize = "boolean?")]
    IsBoolean,
    #[strum(serialize = "procedure?")]
    IsProcedure,
    #[strum(serialize = "char?")]
    IsChar,
    #[strum(serialize = "eq?")]
    Eq,
    #[strum(serialize = "car")]
    Car,
    #[strum(serialize = "cdr")]
    Cdr,
    #[strum(serialize = "symbol->string")]
    SymbolToString,
    #[strum(serialize = "number->string")]
    NumberToString,
    #[strum(serialize = "=")]
    EqNum,
    #[strum(serialize = "<")]
    Lt,
    #[strum(serialize = ">")]
    Gt,
    #[strum(serialize = "<=")]
    Le,
    #[strum(serialize = ">=")]
    Ge,
}

impl Builtin {
    pub fn name(self) -> &'static str {
        self.into()
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::try_from(name).ok()
    }

    pub fn id(self) -> i32 {
        self as usize as i32
    }

    pub fn from_id(id: i32) -> Option<Self> {
        Self::from_repr(id as usize)
    }

    pub fn func_type(self) -> FuncType {
        match self {
            Builtin::Display => FuncType {
                args: ti_vec![Type::Val(ValType::String)], // TODO: 一旦Stringのみ
                rets: ti_vec![Type::Val(ValType::Nil)],
            },
            Builtin::Add => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Int)],
            },
            Builtin::Sub => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Int)],
            },
            Builtin::WriteChar => FuncType {
                args: ti_vec![Type::Val(ValType::Char)],
                rets: ti_vec![Type::Val(ValType::Nil)],
            },
            Builtin::IsPair => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::IsSymbol => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::IsString => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::IsNumber => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::IsBoolean => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::IsProcedure => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::IsChar => FuncType {
                args: ti_vec![Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::Eq => FuncType {
                args: ti_vec![Type::Boxed, Type::Boxed],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::Car => FuncType {
                args: ti_vec![Type::Val(ValType::Cons)],
                rets: ti_vec![Type::Boxed],
            },
            Builtin::Cdr => FuncType {
                args: ti_vec![Type::Val(ValType::Cons)],
                rets: ti_vec![Type::Boxed],
            },
            Builtin::SymbolToString => FuncType {
                args: ti_vec![Type::Val(ValType::Symbol)],
                rets: ti_vec![Type::Val(ValType::String)],
            },
            Builtin::NumberToString => FuncType {
                args: ti_vec![Type::Val(ValType::Int)], // TODO: 一般のnumberに使えるように
                rets: ti_vec![Type::Val(ValType::String)],
            },
            Builtin::EqNum => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::Lt => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::Gt => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::Le => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
            Builtin::Ge => FuncType {
                args: ti_vec![Type::Val(ValType::Int), Type::Val(ValType::Int)],
                rets: ti_vec![Type::Val(ValType::Bool)],
            },
        }
    }
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
    Cons(LocalId, LocalId),
    CreateMutCell(Type),
    DerefMutCell(Type, LocalId),
    SetMutCell(Type, LocalId /* mutcell */, LocalId /* value */),
    Closure(Vec<LocalId>, FuncId),
    CallClosure(bool, LocalId, Vec<LocalId>),
    Move(LocalId),
    Box(ValType, LocalId),
    Unbox(ValType, LocalId),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        LocalId,        /* closure */
        usize,          /* env index */
    ),
    GlobalSet(GlobalId, LocalId),
    GlobalGet(GlobalId),
    // Builtin = BuiltinClosure + CallClosureだが後から最適化するのは大変なので一旦分けておく
    Builtin(Builtin, Vec<LocalId>),
    Error(LocalId),
    InitGlobals(usize), // global count
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
