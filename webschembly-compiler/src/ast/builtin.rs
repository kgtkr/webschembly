use strum_macros::{EnumIter, EnumString, IntoStaticStr};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, EnumString, IntoStaticStr)]
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
    #[strum(serialize = "vector-length")]
    VectorLength,
    #[strum(serialize = "vector-ref")]
    VectorRef,
    #[strum(serialize = "vector-set!")]
    VectorSet,
    #[strum(serialize = "vector?")]
    IsVector,
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

    pub fn typ(self) -> BuiltinType {
        match self {
            Builtin::Display => BuiltinType { args_count: 1 },
            Builtin::Add => BuiltinType { args_count: 2 },
            Builtin::Sub => BuiltinType { args_count: 2 },
            Builtin::WriteChar => BuiltinType { args_count: 1 },
            Builtin::IsPair => BuiltinType { args_count: 1 },
            Builtin::IsSymbol => BuiltinType { args_count: 1 },
            Builtin::IsString => BuiltinType { args_count: 1 },
            Builtin::IsNumber => BuiltinType { args_count: 1 },
            Builtin::IsBoolean => BuiltinType { args_count: 1 },
            Builtin::IsProcedure => BuiltinType { args_count: 1 },
            Builtin::IsChar => BuiltinType { args_count: 1 },
            Builtin::VectorLength => BuiltinType { args_count: 1 },
            Builtin::VectorRef => BuiltinType { args_count: 2 },
            Builtin::VectorSet => BuiltinType { args_count: 3 },
            Builtin::IsVector => BuiltinType { args_count: 1 },
            Builtin::Eq => BuiltinType { args_count: 2 },
            Builtin::Car => BuiltinType { args_count: 1 },
            Builtin::Cdr => BuiltinType { args_count: 1 },
            Builtin::SymbolToString => BuiltinType { args_count: 1 },
            Builtin::NumberToString => BuiltinType { args_count: 1 },
            Builtin::EqNum => BuiltinType { args_count: 2 },
            Builtin::Lt => BuiltinType { args_count: 2 },
            Builtin::Gt => BuiltinType { args_count: 2 },
            Builtin::Le => BuiltinType { args_count: 2 },
            Builtin::Ge => BuiltinType { args_count: 2 },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinType {
    pub args_count: usize,
    // TODO: 可変長引数の処理
}
