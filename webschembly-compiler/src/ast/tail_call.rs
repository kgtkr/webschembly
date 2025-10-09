use super::Defined;
use super::Desugared;
use super::astx::*;
use crate::x::FamilyX;
use crate::x::Phase;
use crate::x::TypeMap;
use crate::x::type_map;

#[derive(Debug, Clone)]
pub enum TailCall {}

impl Phase for TailCall {
    type Prev = Defined;
}

impl FamilyX<TailCall> for AstX {
    type R = ();
}
impl FamilyX<TailCall> for ConstX {
    type R = ();
}
impl FamilyX<TailCall> for DefineX {
    type R = ();
}
impl FamilyX<TailCall> for LambdaX {
    type R = ();
}
impl FamilyX<TailCall> for IfX {
    type R = ();
}

#[derive(Debug, Clone)]
pub struct TailCallCallR {
    pub is_tail: bool,
}

impl FamilyX<TailCall> for CallX {
    type R = TailCallCallR;
}
impl FamilyX<TailCall> for VarX {
    type R = ();
}
impl FamilyX<TailCall> for BeginX {
    type R = ();
}
impl FamilyX<TailCall> for SetX {
    type R = ();
}
impl FamilyX<TailCall> for LetX {
    type R = ();
}

impl FamilyX<TailCall> for VectorX {
    type R = ();
}
impl FamilyX<TailCall> for QuoteX {
    type R = ();
}

impl FamilyX<TailCall> for ConsX {
    type R = ();
}

impl Ast<TailCall> {
    pub fn from_ast(ast: Ast<<TailCall as Phase>::Prev>) -> Self {
        Ast {
            x: ast.x.add(type_map::key::<TailCall>(), ()),
            exprs: ast
                .exprs
                .into_iter()
                .map(|expr| Expr::from_expr(expr, false))
                .collect(),
        }
    }
}

impl Expr<TailCall> {
    fn from_expr(expr: Expr<<TailCall as Phase>::Prev>, is_tail: bool) -> Self {
        match expr {
            Expr::Const(x, lit) => Expr::Const(x.add(type_map::key::<TailCall>(), ()), lit),
            Expr::Var(x, var) => Expr::Var(x.add(type_map::key::<TailCall>(), ()), var),
            Expr::Define(x, _) => x.get_owned(type_map::key::<Defined>()),
            Expr::Lambda(x, lambda) => Expr::Lambda(
                x.add(type_map::key::<TailCall>(), ()),
                Lambda {
                    args: lambda.args,
                    body: Self::from_exprs(lambda.body, true),
                },
            ),
            Expr::If(x, if_) => Expr::If(
                x.add(type_map::key::<TailCall>(), ()),
                If {
                    cond: Box::new(Self::from_expr(*if_.cond, false)),
                    then: Box::new(Self::from_expr(*if_.then, is_tail)),
                    els: Box::new(Self::from_expr(*if_.els, is_tail)),
                },
            ),
            Expr::Call(x, call) => Expr::Call(
                x.add(type_map::key::<TailCall>(), TailCallCallR { is_tail }),
                Call {
                    func: Box::new(Self::from_expr(*call.func, false)),
                    args: Self::from_exprs(call.args, false),
                },
            ),
            Expr::Begin(x, begin) => Expr::Begin(
                x.add(type_map::key::<TailCall>(), ()),
                Begin {
                    exprs: Self::from_exprs(begin.exprs, is_tail),
                },
            ),
            Expr::Set(x, set) => Expr::Set(
                x.add(type_map::key::<TailCall>(), ()),
                Set {
                    name: set.name,
                    expr: Box::new(Self::from_expr(*set.expr, false)),
                },
            ),
            Expr::Let(x, let_) => Expr::Let(
                x.add(type_map::key::<TailCall>(), ()),
                Let {
                    bindings: let_
                        .bindings
                        .into_iter()
                        .map(|(name, expr)| (name, Self::from_expr(expr, false)))
                        .collect(),
                    body: Self::from_exprs(let_.body, is_tail),
                },
            ),
            Expr::Vector(x, vec) => Expr::Vector(x.add(type_map::key::<TailCall>(), ()), {
                vec.into_iter()
                    .map(|expr| Self::from_expr(expr, false))
                    .collect()
            }),
            Expr::Quote(x, _) => x.get_owned(type_map::key::<Desugared>()),
            Expr::Cons(x, cons) => Expr::Cons(
                x.add(type_map::key::<TailCall>(), ()),
                Cons {
                    car: Box::new(Self::from_expr(*cons.car, false)),
                    cdr: Box::new(Self::from_expr(*cons.cdr, false)),
                },
            ),
        }
    }

    fn from_exprs(
        exprs: Vec<Expr<<TailCall as Phase>::Prev>>,
        is_tail: bool,
    ) -> Vec<Expr<TailCall>> {
        let n = exprs.len();
        exprs
            .into_iter()
            .enumerate()
            .map(|(i, expr)| Self::from_expr(expr, is_tail && i == n - 1))
            .collect()
    }
}
