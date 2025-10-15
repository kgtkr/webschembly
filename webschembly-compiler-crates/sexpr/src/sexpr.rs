use ordered_float::NotNan;

use webschembly_compiler_locate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SUVectorKind {
    S64,
    F64,
}

#[derive(Debug, Clone)]
pub enum SExprKind {
    Bool(bool),
    Int(i64),
    Float(NotNan<f64>),
    NaN,
    String(String),
    Char(char),
    Symbol(String),
    Cons(Box<Cons>),
    Vector(Vec<SExpr>),
    UVector(SUVectorKind, Vec<SExpr>),
    Nil,
}

#[derive(Debug, Clone)]
pub struct SExpr {
    pub kind: SExprKind,
    pub span: Span,
}

impl SExpr {
    pub fn to_vec(self) -> Option<Vec<SExpr>> {
        match self.kind {
            SExprKind::Cons(cons) => cons.into_vec(),
            SExprKind::Nil => Some(vec![]),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cons {
    pub car: SExpr,
    pub cdr: SExpr,
}

impl Cons {
    pub fn new(car: SExpr, cdr: SExpr) -> Self {
        Self { car, cdr }
    }

    fn into_vec_and_cdr(self) -> (Vec<SExpr>, SExpr) {
        let mut list = vec![self.car];
        let mut cdr = self.cdr;
        while let SExprKind::Cons(cons) = cdr.kind {
            list.push(cons.car);
            cdr = cons.cdr;
        }
        (list, cdr)
    }

    fn into_vec(self) -> Option<Vec<SExpr>> {
        let (list, cdr) = self.into_vec_and_cdr();
        if let SExprKind::Nil = cdr.kind {
            Some(list)
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! list {
    (=> $span:expr) => {
        $crate::SExpr {kind: $crate::SExprKind::Nil, span: $span}
    };
    (..$cdr:expr) => {
        $cdr
    };
    ($car:expr => $span:expr, $($t:tt)*) => {
        $crate::SExpr {kind: $crate::SExprKind::Cons(Box::new($crate::Cons::new($car, list!($($t)*)))), span: $span}
    };
}

#[macro_export]
macro_rules! list_pattern {
    (=> $span:pat) => {
        $crate::SExpr {kind: $crate::SExprKind::Nil, span: $span, ..}
    };
    () => {
        list_pattern!(=> _)
    };
    (..$cdr:pat) => {
        $cdr
    };
    ($car:pat => $span:pat, $($t:tt)*) => {
        $crate::SExpr {kind: $crate::SExprKind::Cons(box $crate::Cons{car: $car, cdr: list_pattern!($($t)*)}), span: $span, ..}
    };
    ($car:pat, $($t:tt)*) => {
        list_pattern!($car => _, $($t)*)
    };
}
