#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SExpr {
    Bool(bool),
    Int(i64),
    String(String),
    Char(char),
    Symbol(String),
    Cons(Box<Cons>),
    Nil,
}

impl SExpr {
    pub fn to_vec(self) -> Option<Vec<SExpr>> {
        match self {
            SExpr::Cons(cons) => cons.to_vec(),
            SExpr::Nil => Some(vec![]),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cons {
    pub car: SExpr,
    pub cdr: SExpr,
}

impl Cons {
    pub fn new(car: SExpr, cdr: SExpr) -> Self {
        Self { car, cdr }
    }

    fn to_vec_and_cdr(self) -> (Vec<SExpr>, SExpr) {
        let mut list = vec![self.car];
        let mut cdr = self.cdr;
        while let SExpr::Cons(cons) = cdr {
            list.push(cons.car);
            cdr = cons.cdr;
        }
        (list, cdr)
    }

    fn to_vec(self) -> Option<Vec<SExpr>> {
        let (list, cdr) = self.to_vec_and_cdr();
        if cdr == SExpr::Nil {
            Some(list)
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! list {
    () => {
        $crate::sexpr::SExpr::Nil
    };
    (..$cdr:expr) => {
        $cdr
    };
    ($car:expr) => {
        $crate::sexpr::SExpr::Cons(Box::new($crate::sexpr::Cons::new($car, $crate::sexpr::SExpr::Nil)))
    };
    ($car:expr, $($t:tt)*) => {
        $crate::sexpr::SExpr::Cons(Box::new($crate::sexpr::Cons::new($car, list!($($t)*))))
    };
}

#[macro_export]
macro_rules! list_pattern {
    () => {
        $crate::sexpr::SExpr::Nil
    };
    (..$cdr:pat) => {
        $cdr
    };
    ($car:pat) => {
        $crate::sexpr::SExpr::Cons(box $crate::sexpr::Cons{car: $car, cdr: $crate::sexpr::SExpr::Nil})
    };
    ($car:pat, $($t:tt)*) => {
        $crate::sexpr::SExpr::Cons(box $crate::sexpr::Cons{car: $car, cdr: list_pattern!($($t)*)})
    };
}
