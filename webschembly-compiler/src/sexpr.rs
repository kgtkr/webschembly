use crate::span::Span;

#[derive(Debug, Clone)]
pub enum SExprKind {
    Bool(bool),
    Int(i64),
    String(String),
    Char(char),
    Symbol(String),
    Cons(Box<Cons>),
    Vector(Vec<SExpr>),
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
        $crate::sexpr::SExpr {kind: $crate::sexpr::SExprKind::Nil, span: $span}
    };
    (..$cdr:expr) => {
        $cdr
    };
    ($car:expr => $span:expr, $($t:tt)*) => {
        $crate::sexpr::SExpr {kind: $crate::sexpr::SExprKind::Cons(Box::new($crate::sexpr::Cons::new($car, list!($($t)*)))), span: $span}
    };
}

#[macro_export]
macro_rules! list_pattern {
    (=> $span:pat) => {
        $crate::sexpr::SExpr {kind: $crate::sexpr::SExprKind::Nil, span: $span, ..}
    };
    () => {
        list_pattern!(=> _)
    };
    (..$cdr:pat) => {
        $cdr
    };
    ($car:pat => $span:pat, $($t:tt)*) => {
        $crate::sexpr::SExpr {kind: $crate::sexpr::SExprKind::Cons(box $crate::sexpr::Cons{car: $car, cdr: list_pattern!($($t)*)}), span: $span, ..}
    };
    ($car:pat, $($t:tt)*) => {
        list_pattern!($car => _, $($t)*)
    };
}
