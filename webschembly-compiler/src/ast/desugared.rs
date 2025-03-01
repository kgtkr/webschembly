use frunk::field;
use frunk::hlist::h_cons;

use super::ast::*;
use super::Parsed;
use crate::x::FamilyX;
use crate::x::Phase;

#[derive(Debug, Clone)]
pub enum Desugared {}

impl Phase for Desugared {
    type Prev = Parsed;
}

impl FamilyX<Desugared> for AstX {
    type R = ();
}
impl FamilyX<Desugared> for LiteralX {
    type R = ();
}
impl FamilyX<Desugared> for DefineX {
    type R = ();
}
impl FamilyX<Desugared> for LambdaX {
    type R = ();
}
impl FamilyX<Desugared> for IfX {
    type R = ();
}
impl FamilyX<Desugared> for CallX {
    type R = ();
}
impl FamilyX<Desugared> for VarX {
    type R = ();
}
impl FamilyX<Desugared> for BeginX {
    type R = ();
}
impl FamilyX<Desugared> for SetX {
    type R = ();
}

impl FamilyX<Desugared> for LetX {
    type R = !;
}

impl Ast<Desugared> {
    pub fn from_ast(ast: Ast<<Desugared as Phase>::Prev>) -> Self {
        Ast {
            x: h_cons(field![Desugared, ()], ast.x),
            exprs: ast.exprs.into_iter().map(Expr::from_expr).collect(),
        }
    }
}

impl Expr<Desugared> {
    fn from_expr(expr: Expr<<Desugared as Phase>::Prev>) -> Self {
        match expr {
            Expr::Literal(x, lit) => Expr::Literal(h_cons(field![Desugared, ()], x), lit),
            Expr::Var(x, var) => Expr::Var(h_cons(field![Desugared, ()], x), var),
            Expr::Define(x, def) => Expr::Define(
                h_cons(field![Desugared, ()], x),
                Define {
                    name: def.name,
                    expr: Box::new(Self::from_expr(*def.expr)),
                },
            ),
            Expr::Lambda(x, lambda) => Expr::Lambda(
                h_cons(field![Desugared, ()], x),
                Lambda {
                    args: lambda.args,
                    body: lambda.body.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::If(x, if_) => Expr::If(
                h_cons(field![Desugared, ()], x),
                If {
                    cond: Box::new(Self::from_expr(*if_.cond)),
                    then: Box::new(Self::from_expr(*if_.then)),
                    els: Box::new(Self::from_expr(*if_.els)),
                },
            ),
            Expr::Call(x, call) => Expr::Call(
                h_cons(field![Desugared, ()], x),
                Call {
                    func: Box::new(Self::from_expr(*call.func)),
                    args: call.args.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::Begin(x, begin) => Expr::Begin(
                h_cons(field![Desugared, ()], x),
                Begin {
                    exprs: begin.exprs.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::Set(x, set) => Expr::Set(
                h_cons(field![Desugared, ()], x),
                Set {
                    name: set.name,
                    expr: Box::new(Self::from_expr(*set.expr)),
                },
            ),
            Expr::Let(x, let_) => {
                let (names, exprs) = let_.bindings.into_iter().collect::<(Vec<_>, Vec<_>)>();
                Expr::Call(
                    h_cons(field![Desugared, ()], x.clone()),
                    Call {
                        func: Box::new(Expr::Lambda(
                            h_cons(field![Desugared, ()], x),
                            Lambda {
                                args: names,
                                body: let_.body.into_iter().map(Self::from_expr).collect(),
                            },
                        )),
                        args: exprs.into_iter().map(Self::from_expr).collect(),
                    },
                )
            }
        }
    }
}
