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
    type R = !;
}
impl FamilyX<Desugared> for SetX {
    type R = ();
}

impl FamilyX<Desugared> for LetX {
    type R = ();
}

impl FamilyX<Desugared> for LetRecX {
    type R = ();
}

impl FamilyX<Desugared> for VectorX {
    type R = ();
}

impl FamilyX<Desugared> for UVectorX {
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

impl ElementInto<parsed::ParsedUVectorR> for parsed::ParsedQuoteR {
    type Param = ();

    fn element_into(self, _: Self::Param) -> parsed::ParsedUVectorR {
        parsed::ParsedUVectorR { span: self.span }
    }
}

impl Ast<Desugared> {
    pub fn from_ast(ast: Ast<<Desugared as Phase>::Prev>) -> Self {
        Ast {
            x: ast.x.add(type_map::key::<Desugared>(), ()),
            exprs: Expr::from_exprs(ast.exprs),
        }
    }
}

impl Expr<Desugared> {
    fn from_expr(expr: Expr<<Desugared as Phase>::Prev>, exprs: &mut Vec<Expr<Desugared>>) {
        match expr {
            Expr::Const(x, lit) => {
                exprs.push(Expr::Const(x.add(type_map::key::<Desugared>(), ()), lit))
            }
            Expr::Var(x, var) => {
                exprs.push(Expr::Var(x.add(type_map::key::<Desugared>(), ()), var))
            }
            Expr::Define(x, def) => exprs.push(Expr::Define(
                x.add(type_map::key::<Desugared>(), ()),
                Define {
                    name: def.name,
                    expr: Self::from_exprs(def.expr),
                },
            )),
            Expr::Lambda(x, lambda) => exprs.push(Expr::Lambda(
                x.add(type_map::key::<Desugared>(), ()),
                Lambda {
                    args: lambda.args,
                    body: Self::from_exprs(lambda.body),
                },
            )),
            Expr::If(x, if_) => exprs.push(Expr::If(
                x.add(type_map::key::<Desugared>(), ()),
                If {
                    cond: Self::from_exprs(if_.cond),
                    then: Self::from_exprs(if_.then),
                    els: Self::from_exprs(if_.els),
                },
            )),
            Expr::Call(x, call) => exprs.push(Expr::Call(
                x.add(type_map::key::<Desugared>(), ()),
                Call {
                    func: Self::from_exprs(call.func),
                    args: call.args.into_iter().map(Self::from_exprs).collect(),
                },
            )),
            Expr::Begin(_, begin) => {
                for expr in begin.exprs {
                    Self::from_expr(expr, exprs);
                }
            }
            Expr::Set(x, set) => exprs.push(Expr::Set(
                x.add(type_map::key::<Desugared>(), ()),
                Set {
                    name: set.name,
                    expr: Self::from_exprs(set.expr),
                },
            )),
            Expr::Let(x, let_) => exprs.push(Expr::Let(
                x.add(type_map::key::<Desugared>(), ()),
                Let {
                    bindings: let_
                        .bindings
                        .into_iter()
                        .map(|(name, expr)| (name, Self::from_exprs(expr)))
                        .collect(),
                    body: Self::from_exprs(let_.body),
                },
            )),
            Expr::LetRec(x, letrec) => exprs.push(Expr::LetRec(
                x.add(type_map::key::<Desugared>(), ()),
                LetRec {
                    bindings: letrec
                        .bindings
                        .into_iter()
                        .map(|(name, expr)| (name, Self::from_exprs(expr)))
                        .collect(),
                    body: Self::from_exprs(letrec.body),
                },
            )),
            Expr::Vector(x, vec) => exprs.push(Expr::Vector(
                x.add(type_map::key::<Desugared>(), ()),
                vec.into_iter().map(Self::from_exprs).collect(),
            )),
            Expr::UVector(x, uvec) => exprs.push(Expr::UVector(
                x.add(type_map::key::<Desugared>(), ()),
                UVector {
                    kind: uvec.kind,
                    elements: uvec.elements.into_iter().map(Self::from_exprs).collect(),
                },
            )),
            Expr::Quote(x, sexpr) => exprs.push(Self::from_quoted_sexpr(x, sexpr)),
            Expr::Cons(x, cons) => exprs.push(Expr::Cons(
                x.add(type_map::key::<Desugared>(), ()),
                Cons {
                    car: Self::from_exprs(cons.car),
                    cdr: Self::from_exprs(cons.cdr),
                },
            )),
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
            sexpr::SExprKind::Float(f) => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Float(f),
            ),
            sexpr::SExprKind::NaN => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::NaN,
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
                    car: vec![Self::from_quoted_sexpr(x.clone(), cons.car)],
                    cdr: vec![Self::from_quoted_sexpr(x.clone(), cons.cdr)],
                },
            ),
            // TODO: span情報の保持
            sexpr::SExprKind::Vector(vec) => Expr::Vector(
                x.clone()
                    .into_type_map(())
                    .add(type_map::key::<Desugared>(), ()),
                vec.into_iter()
                    .map(|s| vec![Self::from_quoted_sexpr(x.clone(), s)])
                    .collect(),
            ),
            // TODO: span情報の保持
            sexpr::SExprKind::UVector(kind, elements) => Expr::UVector(
                x.clone()
                    .into_type_map(())
                    .add(type_map::key::<Desugared>(), ()),
                UVector {
                    kind: match kind {
                        sexpr::SUVectorKind::S64 => UVectorKind::S64,
                        sexpr::SUVectorKind::F64 => UVectorKind::F64,
                    },
                    elements: elements
                        .into_iter()
                        .map(|s| vec![Self::from_quoted_sexpr(x.clone(), s)])
                        .collect(),
                },
            ),
            sexpr::SExprKind::Nil => Expr::Const(
                x.into_type_map(()).add(type_map::key::<Desugared>(), ()),
                Const::Nil,
            ),
        }
    }

    fn from_exprs(exprs: Vec<Expr<<Desugared as Phase>::Prev>>) -> Vec<Self> {
        let mut result = Vec::new();
        for expr in exprs {
            Self::from_expr(expr, &mut result);
        }
        result
    }
}
