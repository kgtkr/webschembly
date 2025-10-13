use super::Desugared;
use super::astx::*;
use crate::ast::Parsed;
use crate::ast::ParsedLetRecR;
use crate::ast::parsed;
use crate::compiler_error;
use crate::error::Result;
use crate::span::Span;
use crate::x::FamilyX;
use crate::x::Phase;
use crate::x::TypeMap;
use crate::x::type_map;
use crate::x::type_map::ElementInto;
use crate::x::type_map::IntoTypeMap;

// defineをletrec or set!に変換

#[derive(Debug, Clone)]
pub enum Defined {}

impl Phase for Defined {
    type Prev = Desugared;
}

impl FamilyX<Defined> for AstX {
    type R = ();
}
impl FamilyX<Defined> for ConstX {
    type R = ();
}
impl FamilyX<Defined> for DefineX {
    type R = !;
}
impl FamilyX<Defined> for LambdaX {
    type R = ();
}
impl FamilyX<Defined> for IfX {
    type R = ();
}
impl FamilyX<Defined> for CallX {
    type R = ();
}
impl FamilyX<Defined> for VarX {
    type R = ();
}
impl FamilyX<Defined> for BeginX {
    type R = ();
}
impl FamilyX<Defined> for SetX {
    type R = ();
}

impl ElementInto<parsed::ParsedSetR> for parsed::ParsedDefineR {
    type Param = ();

    fn element_into(self, _: Self::Param) -> parsed::ParsedSetR {
        parsed::ParsedSetR {
            span: self.span,
            name_span: self.name_span,
        }
    }
}

impl FamilyX<Defined> for LetX {
    type R = ();
}

impl FamilyX<Defined> for LetRecX {
    type R = ();
}

impl FamilyX<Defined> for VectorX {
    type R = ();
}
impl FamilyX<Defined> for UVectorX {
    type R = ();
}
impl FamilyX<Defined> for QuoteX {
    type R = ();
}

impl FamilyX<Defined> for ConsX {
    type R = ();
}

impl Ast<Defined> {
    pub fn from_ast(ast: Ast<<Defined as Phase>::Prev>) -> Result<Self> {
        let new_exprs: Vec<Expr<Defined>> =
            Expr::<Defined>::from_exprs(ast.exprs, DefineContext::Global, &mut Vec::new())?;
        Ok(Ast {
            x: ast.x.add(type_map::key::<Defined>(), ()),
            exprs: new_exprs,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum DefineContext {
    Global,
    LocalDefinable,
    LocalUndefinable,
}

impl DefineContext {
    fn to_undefinable_if_local(self) -> Self {
        match self {
            DefineContext::Global => DefineContext::Global,
            DefineContext::LocalDefinable => DefineContext::LocalUndefinable,
            DefineContext::LocalUndefinable => DefineContext::LocalUndefinable,
        }
    }
}

impl Expr<Defined> {
    fn from_expr(
        expr: Expr<<Defined as Phase>::Prev>,
        ctx: DefineContext,
        defines: &mut Vec<(String, Span, Vec<Self>)>,
        result: &mut Vec<Self>,
    ) -> Result<()> {
        match expr {
            Expr::Const(x, lit) => {
                result.push(Expr::Const(x.add(type_map::key::<Defined>(), ()), lit));
                Ok(())
            }
            Expr::Var(x, var) => {
                result.push(Expr::Var(x.add(type_map::key::<Defined>(), ()), var));
                Ok(())
            }
            Expr::Define(x, def) => {
                match ctx {
                    DefineContext::Global => {
                        result.push(Expr::Set(
                            // global contextではdefineはset!に変換される
                            x.into_type_map(()).add(type_map::key::<Defined>(), ()),
                            Set {
                                name: def.name,
                                expr: Self::from_exprs(
                                    def.expr,
                                    ctx.to_undefinable_if_local(),
                                    defines,
                                )?,
                            },
                        ));
                        Ok(())
                    }
                    DefineContext::LocalDefinable => {
                        let def_expr =
                            Self::from_exprs(def.expr, ctx.to_undefinable_if_local(), defines)?;
                        defines.push((
                            def.name.clone(),
                            x.get_ref(type_map::key::<Parsed>()).name_span,
                            def_expr,
                        ));
                        Ok(())
                    }
                    DefineContext::LocalUndefinable => Err(compiler_error!(
                        "Define is not allowed in this context: {}",
                        def.name
                    )),
                }
            }
            Expr::Lambda(x, lambda) => {
                let new_body = Self::from_exprs_new_scope(
                    x.get_ref(type_map::key::<Parsed>()).span,
                    lambda.body,
                )?;
                result.push(Expr::Lambda(
                    x.add(type_map::key::<Defined>(), ()),
                    Lambda {
                        args: lambda.args,
                        body: new_body,
                    },
                ));
                Ok(())
            }
            Expr::If(x, if_) => {
                result.push(Expr::If(
                    x.add(type_map::key::<Defined>(), ()),
                    If {
                        cond: Self::from_exprs(if_.cond, ctx.to_undefinable_if_local(), defines)?,
                        then: Self::from_exprs(if_.then, ctx.to_undefinable_if_local(), defines)?,
                        els: Self::from_exprs(if_.els, ctx.to_undefinable_if_local(), defines)?,
                    },
                ));
                Ok(())
            }
            Expr::Call(x, call) => {
                let new_func = Self::from_exprs(call.func, ctx.to_undefinable_if_local(), defines)?;
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_exprs(arg, ctx.to_undefinable_if_local(), defines))
                    .collect::<Result<Vec<_>>>()?;
                result.push(Expr::Call(
                    x.add(type_map::key::<Defined>(), ()),
                    Call {
                        func: new_func,
                        args: new_args,
                    },
                ));
                Ok(())
            }
            Expr::Begin(x, _) => x.get_owned(type_map::key::<Desugared>()),
            Expr::Set(x, set) => {
                let new_expr = Self::from_exprs(set.expr, ctx.to_undefinable_if_local(), defines)?;
                result.push(Expr::Set(
                    x.add(type_map::key::<Defined>(), ()),
                    Set {
                        name: set.name,
                        expr: new_expr,
                    },
                ));
                Ok(())
            }
            Expr::Let(x, let_) => {
                let new_body = Self::from_exprs_new_scope(
                    x.get_ref(type_map::key::<Parsed>()).span,
                    let_.body,
                )?;
                result.push(Expr::Let(
                    x.add(type_map::key::<Defined>(), ()),
                    Let {
                        bindings: let_
                            .bindings
                            .into_iter()
                            .map(|(name, expr)| {
                                Self::from_exprs(expr, ctx.to_undefinable_if_local(), defines)
                                    .map(|expr| (name, expr))
                            })
                            .collect::<Result<Vec<_>>>()?,
                        body: new_body,
                    },
                ));
                Ok(())
            }
            Expr::LetRec(x, letrec) => {
                let new_body = Self::from_exprs_new_scope(
                    x.get_ref(type_map::key::<Parsed>()).span,
                    letrec.body,
                )?;
                result.push(Expr::LetRec(
                    x.add(type_map::key::<Defined>(), ()),
                    LetRec {
                        bindings: letrec
                            .bindings
                            .into_iter()
                            .map(|(name, expr)| {
                                Self::from_exprs(expr, ctx.to_undefinable_if_local(), defines)
                                    .map(|expr| (name, expr))
                            })
                            .collect::<Result<Vec<_>>>()?,
                        body: new_body,
                    },
                ));
                Ok(())
            }
            Expr::Vector(x, vec) => {
                result.push(Expr::Vector(
                    x.add(type_map::key::<Defined>(), ()),
                    vec.into_iter()
                        .map(|v| Self::from_exprs(v, ctx.to_undefinable_if_local(), defines))
                        .collect::<Result<Vec<_>>>()?,
                ));
                Ok(())
            }
            Expr::UVector(x, uvec) => {
                result.push(Expr::UVector(
                    x.add(type_map::key::<Defined>(), ()),
                    UVector {
                        kind: uvec.kind,
                        elements: uvec
                            .elements
                            .into_iter()
                            .map(|v| Self::from_exprs(v, ctx.to_undefinable_if_local(), defines))
                            .collect::<Result<Vec<_>>>()?,
                    },
                ));
                Ok(())
            }
            Expr::Quote(x, _) => x.get_owned(type_map::key::<Desugared>()),
            Expr::Cons(x, cons) => {
                result.push(Expr::Cons(
                    x.add(type_map::key::<Defined>(), ()),
                    Cons {
                        car: Self::from_exprs(cons.car, ctx.to_undefinable_if_local(), defines)?,
                        cdr: Self::from_exprs(cons.cdr, ctx.to_undefinable_if_local(), defines)?,
                    },
                ));
                Ok(())
            }
        }
    }

    fn from_exprs(
        exprs: Vec<Expr<<Defined as Phase>::Prev>>,
        mut ctx: DefineContext,
        defines: &mut Vec<(String, Span, Vec<Self>)>,
    ) -> Result<Vec<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            let is_define = matches!(expr, Expr::Define(_, _));
            Self::from_expr(expr, ctx, defines, &mut result)?;
            if !is_define {
                // defineは先頭に連続して出現しないといけない
                // beginは例外だが、desugerの時点でbeginは消されているので考慮しなくて良い
                ctx = ctx.to_undefinable_if_local();
            }
        }
        Ok(result)
    }

    // スコープを作る命令
    // 一つでもdefineがあれば全体をletrecで囲む
    fn from_exprs_new_scope(
        span: Span,
        exprs: Vec<Expr<<Defined as Phase>::Prev>>,
    ) -> Result<Vec<Self>> {
        let mut defines = Vec::new();
        let exprs = Self::from_exprs(exprs, DefineContext::LocalDefinable, &mut defines)?;
        if defines.is_empty() {
            Ok(exprs)
        } else {
            let binding_name_spans = defines
                .iter()
                .map(|(_, name_span, _)| *name_span)
                .collect::<Vec<_>>();
            let bindings = defines
                .into_iter()
                .map(|(name, _, expr)| (name, expr))
                .collect::<Vec<_>>();
            Ok(vec![Expr::LetRec(
                type_map::empty()
                    .add(
                        type_map::key::<Parsed>(),
                        ParsedLetRecR {
                            span,
                            binding_name_spans,
                        },
                    )
                    .add(type_map::key::<Desugared>(), ())
                    .add(type_map::key::<Defined>(), ()),
                LetRec {
                    bindings,
                    body: exprs,
                },
            )])
        }
    }
}
