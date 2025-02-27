use super::ast::*;
use super::Parsed;
use crate::x::FamilyX;

#[derive(Debug, Clone)]
pub enum Desugared {}

type Prev = Parsed;

impl FamilyX<Desugared> for AstX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for LiteralX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for DefineX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for LambdaX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for IfX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for CallX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for VarX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for BeginX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Desugared> for SetX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl FamilyX<Desugared> for LetX {
    type R = !;
}

impl Ast<Desugared> {
    pub fn from_ast(ast: Ast<Prev>) -> Self {
        Ast {
            x: ast.x,
            exprs: ast.exprs.into_iter().map(Expr::from_expr).collect(),
        }
    }
}

impl Expr<Desugared> {
    fn from_expr(expr: Expr<Prev>) -> Self {
        match expr {
            Expr::Literal(x, lit) => Expr::Literal(x, lit),
            Expr::Var(x, var) => Expr::Var(x, var),
            Expr::Define(x, def) => Expr::Define(
                x,
                Define {
                    name: def.name,
                    expr: Box::new(Self::from_expr(*def.expr)),
                },
            ),
            Expr::Lambda(x, lambda) => Expr::Lambda(
                x,
                Lambda {
                    args: lambda.args,
                    body: lambda.body.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::If(x, if_) => Expr::If(
                x,
                If {
                    cond: Box::new(Self::from_expr(*if_.cond)),
                    then: Box::new(Self::from_expr(*if_.then)),
                    els: Box::new(Self::from_expr(*if_.els)),
                },
            ),
            Expr::Call(x, call) => Expr::Call(
                x,
                Call {
                    func: Box::new(Self::from_expr(*call.func)),
                    args: call.args.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::Begin(x, begin) => Expr::Begin(
                x,
                Begin {
                    exprs: begin.exprs.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::Set(x, set) => Expr::Set(
                x,
                Set {
                    name: set.name,
                    expr: Box::new(Self::from_expr(*set.expr)),
                },
            ),
            Expr::Let(x, let_) => {
                let (names, exprs) = let_.bindings.into_iter().collect::<(Vec<_>, Vec<_>)>();
                Expr::Call(
                    x,
                    Call {
                        func: Box::new(Expr::Lambda(
                            (),
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
