use super::ast::*;
use super::parsed;
use super::Parsed;
use crate::x::type_map;
use crate::x::type_map::IntoTypeMap;
use crate::x::FamilyX;
use crate::x::Phase;
use crate::x::TypeMap;

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

impl From<parsed::ParsedLetR> for parsed::ParsedLambdaR {
    fn from(val: parsed::ParsedLetR) -> Self {
        parsed::ParsedLambdaR {
            span: val.span,
            arg_spans: val.binding_spans,
        }
    }
}

impl From<parsed::ParsedLetR> for parsed::ParsedCallR {
    fn from(val: parsed::ParsedLetR) -> Self {
        parsed::ParsedCallR { span: val.span }
    }
}

impl From<parsed::ParsedDefineR> for parsed::ParsedSetR {
    fn from(val: parsed::ParsedDefineR) -> Self {
        parsed::ParsedSetR {
            span: val.span,
            name_span: val.name_span,
        }
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
            Expr::Literal(x, lit) => Expr::Literal(x.add(type_map::key::<Desugared>(), ()), lit),
            Expr::Var(x, var) => Expr::Var(x.add(type_map::key::<Desugared>(), ()), var),
            Expr::Define(x, def) => Expr::Define(
                x.add(type_map::key::<Desugared>(), ()),
                Define {
                    name: def.name,
                    expr: Box::new(Self::from_expr(*def.expr)),
                },
            ),
            Expr::Lambda(x, lambda) => Expr::Lambda(
                x.add(type_map::key::<Desugared>(), ()),
                Lambda {
                    args: lambda.args,
                    body: lambda.body.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::If(x, if_) => Expr::If(
                x.add(type_map::key::<Desugared>(), ()),
                If {
                    cond: Box::new(Self::from_expr(*if_.cond)),
                    then: Box::new(Self::from_expr(*if_.then)),
                    els: Box::new(Self::from_expr(*if_.els)),
                },
            ),
            Expr::Call(x, call) => Expr::Call(
                x.add(type_map::key::<Desugared>(), ()),
                Call {
                    func: Box::new(Self::from_expr(*call.func)),
                    args: call.args.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::Begin(x, begin) => Expr::Begin(
                x.add(type_map::key::<Desugared>(), ()),
                Begin {
                    exprs: begin.exprs.into_iter().map(Self::from_expr).collect(),
                },
            ),
            Expr::Set(x, set) => Expr::Set(
                x.add(type_map::key::<Desugared>(), ()),
                Set {
                    name: set.name,
                    expr: Box::new(Self::from_expr(*set.expr)),
                },
            ),
            Expr::Let(x, let_) => {
                let (names, exprs) = let_.bindings.into_iter().collect::<(Vec<_>, Vec<_>)>();
                Expr::Call(
                    x.clone()
                        .into_type_map()
                        .add(type_map::key::<Desugared>(), ()),
                    Call {
                        func: Box::new(Expr::Lambda(
                            x.into_type_map().add(type_map::key::<Desugared>(), ()),
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
