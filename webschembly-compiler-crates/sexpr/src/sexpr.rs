use ordered_float::NotNan;

use webschembly_compiler_locate::L;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SUVectorKind {
    S64,
    F64,
}

#[derive(Debug, Clone)]
pub enum SExpr {
    Bool(bool),
    Int(i64),
    Float(NotNan<f64>),
    NaN,
    String(String),
    Char(char),
    Symbol(String),
    Cons(Box<Cons>),
    Vector(Vec<LSExpr>),
    UVector(SUVectorKind, Vec<LSExpr>),
    Nil,
}

impl SExpr {
    pub fn to_vec_and_cdr(lsexpr: LSExpr) -> (Vec<LSExpr>, LSExpr) {
        match lsexpr.value {
            SExpr::Cons(cons) => cons.into_vec_and_cdr(),
            _ => (vec![], lsexpr),
        }
    }

    pub fn to_vec(self) -> Option<Vec<LSExpr>> {
        match self {
            SExpr::Cons(cons) => cons.into_vec(),
            SExpr::Nil => Some(vec![]),
            _ => None,
        }
    }
}

pub type LSExpr = L<SExpr>;

#[derive(Debug, Clone)]
pub struct Cons {
    pub car: LSExpr,
    pub cdr: LSExpr,
}

impl Cons {
    pub fn new(car: LSExpr, cdr: LSExpr) -> Self {
        Self { car, cdr }
    }

    fn into_vec_and_cdr(self) -> (Vec<LSExpr>, LSExpr) {
        let mut list = vec![self.car];
        let mut cdr = self.cdr;
        while let SExpr::Cons(cons) = cdr.value {
            list.push(cons.car);
            cdr = cons.cdr;
        }
        (list, cdr)
    }

    fn into_vec(self) -> Option<Vec<LSExpr>> {
        let (list, cdr) = self.into_vec_and_cdr();
        if let SExpr::Nil = cdr.value {
            Some(list)
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! list {
    (=> $span:expr) => {
        $crate::LSExpr {value: $crate::SExpr::Nil, span: $span}
    };
    (..$cdr:expr) => {
        $cdr
    };
    ($car:expr => $span:expr, $($t:tt)*) => {
        $crate::LSExpr {value: $crate::SExpr::Cons(Box::new($crate::Cons::new($car, list!($($t)*)))), span: $span}
    };
}

#[macro_export]
macro_rules! list_pattern {
    (=> $span:pat) => {
        $crate::LSExpr {value: $crate::SExpr::Nil, span: $span, ..}
    };
    () => {
        list_pattern!(=> _)
    };
    (..$cdr:pat) => {
        $cdr
    };
    ($car:pat => $span:pat, $($t:tt)*) => {
        $crate::LSExpr {value: $crate::SExpr::Cons(box $crate::Cons{car: $car, cdr: list_pattern!($($t)*)}), span: $span, ..}
    };
    ($car:pat, $($t:tt)*) => {
        list_pattern!($car => _, $($t)*)
    };
}
