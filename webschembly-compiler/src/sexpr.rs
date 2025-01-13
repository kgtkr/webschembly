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

    pub fn to_list(self) -> List {
        let mut list = vec![self.car];
        let mut cdr = self.cdr;
        loop {
            match cdr {
                SExpr::Cons(cons) => {
                    list.push(cons.car);
                    cdr = cons.cdr;
                }
                SExpr::Nil => return List::List(list),
                cdr => return List::DottedList(list, cdr),
            }
        }
    }

    pub fn from_list(list: List) -> Self {
        let (mut list, cdr) = match list {
            List::List(list) => (list, SExpr::Nil),
            List::DottedList(list, cdr) => (list, cdr),
        };

        let mut cons = Cons::new(list.pop().unwrap(), cdr);
        for car in list.into_iter().rev() {
            cons = Cons::new(car, SExpr::Cons(Box::new(cons)));
        }
        cons
    }
}

#[derive(Debug, Clone)]
pub enum List {
    List(Vec<SExpr>),
    // cdr is not a list
    DottedList(Vec<SExpr>, SExpr),
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
