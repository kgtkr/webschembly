use strum_macros::{EnumIter, EnumString, IntoStaticStr};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, EnumString, IntoStaticStr)]
pub enum Builtin {
    #[strum(serialize = "display")]
    Display, // TODO: 将来的には組み込み関数ではなくしたい
    #[strum(serialize = "+")]
    Add,
    #[strum(serialize = "-")]
    Sub,
    #[strum(serialize = "*")]
    Mul,
    #[strum(serialize = "/")]
    Div,
    #[strum(serialize = "quotient")]
    Quotient,
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
    #[strum(serialize = "make-vector")]
    MakeVector,
    #[strum(serialize = "uvector-length")]
    UVectorLength,
    #[strum(serialize = "uvector-ref")]
    UVectorRef,
    #[strum(serialize = "uvector-set!")]
    UVectorSet,
    #[strum(serialize = "uvector?")]
    IsUVector,
    #[strum(serialize = "s64vector?")]
    IsS64Vector,
    #[strum(serialize = "f64vector?")]
    IsF64Vector,
    #[strum(serialize = "make-s64vector")]
    MakeS64Vector,
    #[strum(serialize = "make-f64vector")]
    MakeF64Vector,
    #[strum(serialize = "eq?")]
    Eq,
    #[strum(serialize = "cons")]
    Cons,
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
}
