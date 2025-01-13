#[derive(Debug, Clone)]
pub enum SExpr {
    Bool(bool),
    Int(i64),
    String(String),
    Symbol(String),
    Cons(Box<Cons>),
    Nil,
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

    pub fn to_vec_and_cdr(self) -> (Vec<SExpr>, SExpr) {
        let mut list = vec![self.car];
        let mut cdr = self.cdr;
        while let SExpr::Cons(cons) = cdr {
            list.push(cons.car);
            cdr = cons.cdr;
        }
        (list, cdr)
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
    ($car:expr) => {
        $crate::sexpr::SExpr::Cons(Box::new($crate::sexpr::Cons::new($car, $crate::sexpr::SExpr::Nil)))
    };
    ($car:expr, $($cdr:expr),*) => {
        $crate::sexpr::SExpr::Cons(Box::new($crate::sexpr::Cons::new($car, list!($($cdr),*))))
    };
}
