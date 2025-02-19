#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SExpr {
    Bool(bool),
    Int(i32),
    String(String),
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

    pub fn from_vec(mut list: Vec<SExpr>) -> Self {
        if list.is_empty() {
            SExpr::Nil
        } else {
            let first = list.remove(0);
            SExpr::Cons(Box::new(Cons::from_non_empty_list(NonEmptyList::List(
                first, list,
            ))))
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

    pub fn to_vec_and_cdr(self) -> (Vec<SExpr>, SExpr) {
        let mut list = vec![self.car];
        let mut cdr = self.cdr;
        while let SExpr::Cons(cons) = cdr {
            list.push(cons.car);
            cdr = cons.cdr;
        }
        (list, cdr)
    }

    pub fn to_vec(self) -> Option<Vec<SExpr>> {
        let (list, cdr) = self.to_vec_and_cdr();
        if cdr == SExpr::Nil {
            Some(list)
        } else {
            None
        }
    }

    pub fn to_non_empty_list(self) -> NonEmptyList {
        NonEmptyList::new(self.car, vec![], self.cdr)
    }

    pub fn from_non_empty_list(list: NonEmptyList) -> Self {
        let (car, mut list, mut cdr) = match list {
            NonEmptyList::List(car, list) => (car, list, SExpr::Nil),
            NonEmptyList::DottedList(car, list, cdr) => (car, list, cdr),
        };

        while let Some(car) = list.pop() {
            cdr = SExpr::Cons(Box::new(Cons::new(car, cdr)));
        }
        Cons::new(car, cdr)
    }
}

#[derive(Debug, Clone)]
pub enum NonEmptyList {
    List(SExpr, Vec<SExpr>),
    // cdr is not a cons or nil
    DottedList(SExpr, Vec<SExpr>, SExpr),
}

impl NonEmptyList {
    pub fn new(first: SExpr, middle: Vec<SExpr>, last: SExpr) -> Self {
        match last {
            SExpr::Cons(cons) => {
                let (middle2, cdr) = cons.to_vec_and_cdr();
                let mut middle = middle;
                middle.extend(middle2);
                NonEmptyList::new(first, middle, cdr)
            }
            SExpr::Nil => NonEmptyList::List(first, middle),
            last => NonEmptyList::DottedList(first, middle, last),
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
