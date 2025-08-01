use super::Parsed;
use super::astx::*;
use super::parsed;
use crate::sexpr;
use crate::x::FamilyX;
use crate::x::Phase;
use crate::x::RunX;
use crate::x::TypeMap;
use crate::x::type_map;
use crate::x::type_map::ElementInto;
use crate::x::type_map::IntoTypeMap;

#[derive(Debug, Clone)]
pub enum Desugared {}

impl Phase for Desugared {
    type Prev = Parsed;
}

impl FamilyX<Desugared> for AstX {
    type R = ();
}
impl FamilyX<Desugared> for ConstX {
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
    type R = ();
}

impl FamilyX<Desugared> for VectorX {
    type R = ();
}

impl FamilyX<Desugared> for QuoteX {
    type R = !;
}

impl FamilyX<Desugared> for ConsX {
    type R = ();
}

impl ElementInto<parsed::ParsedCallR> for parsed::ParsedLetR {
    type Param = ();

    fn element_into(self, _: Self::Param) -> parsed::ParsedCallR {
        parsed::ParsedCallR { span: self.span }
    }
}

impl ElementInto<parsed::ParsedConstR> for parsed::ParsedQuoteR {
    type Param = ();

    fn element_into(self, _: Self::Param) -> parsed::ParsedConstR {
        parsed::ParsedConstR { span: self.span }
    }
}

impl ElementInto<parsed::ParsedConsR> for parsed::ParsedQuoteR {
    type Param = ();

    fn element_into(self, _: Self::Param) -> parsed::ParsedConsR {
        parsed::ParsedConsR { span: self.span }
    }
}

impl ElementInto<parsed::ParsedVectorR> for parsed::ParsedQuoteR {
    type Param = ();

    fn element_into(self, _: Self::Param) -> parsed::ParsedVectorR {
        parsed::ParsedVectorR { span: self.span }
    }
}

impl Ast<Desugared> {
    pub fn from_ast(ast: Ast<<Desugared as Phase>::Prev>) -> Self {
        Ast {
            x: ast.x.add(type_map::key::<Desugared>(), ()),
            exprs: ast.exprs.into_iter().map(Expr::from_expr).collect(),
        }
    }
}

impl Expr<Desugared> {
    fn from_expr(expr: Expr<<Desugared as Phase>::Prev>) -> Self {
        match expr {
            Expr::Const(x, lit) => Expr::Const(x.add(type_map::key::<Desugared>(), ()), lit),
            Expr::Var(x, var) => Expr::Var(x.add(type_map::key::<Desugared>(), ()), var),
            Expr::Define(x, def) => Expr::Define(x.add(type_map::key::<Desugared>(), ()), Define {
                name: def.name,
                expr: Box::new(Self::from_expr(*def.expr)),
            }),
            Expr::Lambda(x, lambda) => {
                Expr::Lambda(x.add(type_map::key::<Desugared>(), ()), Lambda {
                    args: lambda.args,
                    body: lambda.body.into_iter().map(Self::from_expr).collect(),
                })
            }
            Expr::If(x, if_) => Expr::If(x.add(type_map::key::<Desugared>(), ()), If {
                cond: Box::new(Self::from_expr(*if_.cond)),
                then: Box::new(Self::from_expr(*if_.then)),
                els: Box::new(Self::from_expr(*if_.els)),
            }),
            Expr::Call(x, call) => Expr::Call(x.add(type_map::key::<Desugared>(), ()), Call {
                func: Box::new(Self::from_expr(*call.func)),
                args: call.args.into_iter().map(Self::from_expr).collect(),
            }),
            Expr::Begin(x, begin) => Expr::Begin(x.add(type_map::key::<Desugared>(), ()), Begin {
                exprs: begin.exprs.into_iter().map(Self::from_expr).collect(),
            }),
            Expr::Set(x, set) => Expr::Set(x.add(type_map::key::<Desugared>(), ()), Set {
                name: set.name,
                expr: Box::new(Self::from_expr(*set.expr)),
            }),
            Expr::Let(x, let_) => Expr::Let(x.add(type_map::key::<Desugared>(), ()), Let {
                bindings: let_
                    .bindings
                    .into_iter()
                    .map(|(name, expr)| (name, Self::from_expr(expr)))
                    .collect(),
                body: let_
                    .body
                    .into_iter()
                    .map(Self::from_expr)
                    .collect::<Vec<_>>(),
            }),
            Expr::Vector(x, vec) => Expr::Vector(
                x.add(type_map::key::<Desugared>(), ()),
                vec.into_iter().map(Self::from_expr).collect(),
            ),
            Expr::Quote(x, sexpr) => Self::from_quoted_sexpr(x, sexpr),
            Expr::Cons(x, cons) => Expr::Cons(x.add(type_map::key::<Desugared>(), ()), Cons {
                car: Box::new(Self::from_expr(*cons.car)),
                cdr: Box::new(Self::from_expr(*cons.cdr)),
            }),
        }
    }

    fn from_quoted_sexpr(x: RunX<QuoteX, <Desugared as Phase>::Prev>, sexpr: sexpr::SExpr) -> Self {
        match sexpr.kind {
            sexpr::SExprKind::Bool(b) => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Bool(b),
            ),
            sexpr::SExprKind::Int(i) => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Int(i),
            ),
            sexpr::SExprKind::String(s) => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::String(s),
            ),
            sexpr::SExprKind::Char(c) => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Char(c),
            ),
            sexpr::SExprKind::Symbol(s) => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Symbol(s),
            ),
            // TODO: span情報の保持
            sexpr::SExprKind::Cons(cons) => Expr::Cons(
                x.clone()
                    .into_type_map(())
                    .add(type_map::key::<Desugared>(), ()),
                Cons {
                    car: Box::new(Self::from_quoted_sexpr(x.clone(), cons.car)),
                    cdr: Box::new(Self::from_quoted_sexpr(x.clone(), cons.cdr)),
                },
            ),
            // TODO: span情報の保持
            sexpr::SExprKind::Vector(vec) => Expr::Vector(
                x.clone()
                    .into_type_map(())
                    .add(type_map::key::<Desugared>(), ()),
                vec.into_iter()
                    .map(|s| Self::from_quoted_sexpr(x.clone(), s))
                    .collect(),
            ),
            sexpr::SExprKind::Nil => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Nil,
            ),
        }
    }
}
