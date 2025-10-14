use super::astx::*;
use crate::compiler_error;
use crate::error::Result;
use crate::span::Span;
use crate::x::FamilyX;
use crate::x::Phase;

// defineをletrec or set!に変換

#[derive(Debug, Clone)]
pub enum Defined {}

impl Phase for Defined {}

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
    type R = !;
}
impl FamilyX<Defined> for SetX {
    type R = ();
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
    type R = !;
}

impl FamilyX<Defined> for ConsX {
    type R = ();
}

pub trait DefinedPrevPhase = XBound
where
    BeginX: FamilyX<Self, R = !>,
    QuoteX: FamilyX<Self, R = !>;
type SelfExpr = Expr<Defined>;
impl Ast<Defined> {
    pub fn from_ast<P: DefinedPrevPhase>(ast: Ast<P>) -> Result<Self> {
        let new_exprs = LExpr::from_exprs(ast.exprs, DefineContext::Global, &mut Vec::new())?;
        Ok(Ast {
            x: (),
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

impl LExpr<Defined> {
    fn from_expr<P: DefinedPrevPhase>(
        expr: LExpr<P>,
        ctx: DefineContext,
        defines: &mut Vec<Located<Binding<Defined>>>,
        result: &mut Vec<Self>,
    ) -> Result<()> {
        match expr.value {
            Expr::Const(_, lit) => {
                result.push(SelfExpr::Const((), lit).with_span(expr.span));
                Ok(())
            }
            Expr::Var(_, var) => {
                result.push(SelfExpr::Var((), var).with_span(expr.span));
                Ok(())
            }
            Expr::Define(_, def) => {
                match ctx {
                    DefineContext::Global => {
                        result.push(
                            SelfExpr::Set(
                                // global contextではdefineはset!に変換される
                                (),
                                Set {
                                    name: def.name,
                                    expr: Self::from_exprs(
                                        def.expr,
                                        ctx.to_undefinable_if_local(),
                                        defines,
                                    )?,
                                },
                            )
                            .with_span(expr.span),
                        );
                        Ok(())
                    }
                    DefineContext::LocalDefinable => {
                        let def_expr =
                            Self::from_exprs(def.expr, ctx.to_undefinable_if_local(), defines)?;
                        defines.push(
                            Binding {
                                name: def.name,
                                expr: def_expr,
                            }
                            .with_span(expr.span),
                        );
                        Ok(())
                    }
                    DefineContext::LocalUndefinable => Err(compiler_error!(
                        "Define is not allowed in this context: {}",
                        def.name.value
                    )),
                }
            }
            Expr::Lambda(_, lambda) => {
                let new_body = Self::from_exprs_new_scope(expr.span, lambda.body)?;
                result.push(
                    SelfExpr::Lambda(
                        (),
                        Lambda {
                            args: lambda.args,
                            body: new_body,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::If(_, if_) => {
                result.push(
                    SelfExpr::If(
                        (),
                        If {
                            cond: Self::from_exprs(
                                if_.cond,
                                ctx.to_undefinable_if_local(),
                                defines,
                            )?,
                            then: Self::from_exprs(
                                if_.then,
                                ctx.to_undefinable_if_local(),
                                defines,
                            )?,
                            els: Self::from_exprs(if_.els, ctx.to_undefinable_if_local(), defines)?,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::Call(_, call) => {
                let new_func = Self::from_exprs(call.func, ctx.to_undefinable_if_local(), defines)?;
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_exprs(arg, ctx.to_undefinable_if_local(), defines))
                    .collect::<Result<Vec<_>>>()?;
                result.push(
                    SelfExpr::Call(
                        (),
                        Call {
                            func: new_func,
                            args: new_args,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::Begin(x, _) => x,
            Expr::Set(_, set) => {
                let new_expr = Self::from_exprs(set.expr, ctx.to_undefinable_if_local(), defines)?;
                result.push(
                    SelfExpr::Set(
                        (),
                        Set {
                            name: set.name,
                            expr: new_expr,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::Let(_, let_) => {
                let new_body = Self::from_exprs_new_scope(expr.span, let_.body)?;
                result.push(
                    SelfExpr::Let(
                        (),
                        Let {
                            bindings: let_
                                .bindings
                                .into_iter()
                                .map(|binding| {
                                    Ok(Binding {
                                        name: binding.value.name,
                                        expr: Self::from_exprs(
                                            binding.value.expr,
                                            ctx.to_undefinable_if_local(),
                                            defines,
                                        )?,
                                    }
                                    .with_span(binding.span))
                                })
                                .collect::<Result<Vec<_>>>()?,
                            body: new_body,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::LetRec(_, letrec) => {
                let new_body = Self::from_exprs_new_scope(expr.span, letrec.body)?;
                result.push(
                    SelfExpr::LetRec(
                        (),
                        LetRec {
                            bindings: letrec
                                .bindings
                                .into_iter()
                                .map(|binding| {
                                    Ok(Binding {
                                        name: binding.value.name,
                                        expr: Self::from_exprs(
                                            binding.value.expr,
                                            ctx.to_undefinable_if_local(),
                                            defines,
                                        )?,
                                    }
                                    .with_span(binding.span))
                                })
                                .collect::<Result<Vec<_>>>()?,
                            body: new_body,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::Vector(_, vec) => {
                result.push(
                    SelfExpr::Vector(
                        (),
                        vec.into_iter()
                            .map(|v| Self::from_exprs(v, ctx.to_undefinable_if_local(), defines))
                            .collect::<Result<Vec<_>>>()?,
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::UVector(_, uvec) => {
                result.push(
                    SelfExpr::UVector(
                        (),
                        UVector {
                            kind: uvec.kind,
                            elements: uvec
                                .elements
                                .into_iter()
                                .map(|v| {
                                    Self::from_exprs(v, ctx.to_undefinable_if_local(), defines)
                                })
                                .collect::<Result<Vec<_>>>()?,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::Quote(x, _) => x,
            Expr::Cons(_, cons) => {
                result.push(
                    SelfExpr::Cons(
                        (),
                        Cons {
                            car: Self::from_exprs(
                                cons.car,
                                ctx.to_undefinable_if_local(),
                                defines,
                            )?,
                            cdr: Self::from_exprs(
                                cons.cdr,
                                ctx.to_undefinable_if_local(),
                                defines,
                            )?,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
        }
    }

    fn from_exprs<P: DefinedPrevPhase>(
        exprs: Vec<LExpr<P>>,
        mut ctx: DefineContext,
        defines: &mut Vec<Located<Binding<Defined>>>,
    ) -> Result<Vec<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            let is_define = matches!(expr.value, Expr::Define(_, _));
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
    fn from_exprs_new_scope<P: DefinedPrevPhase>(
        span: Span,
        exprs: Vec<LExpr<P>>,
    ) -> Result<Vec<Self>> {
        let mut defines = Vec::new();
        let exprs = Self::from_exprs(exprs, DefineContext::LocalDefinable, &mut defines)?;
        if defines.is_empty() {
            Ok(exprs)
        } else {
            Ok(vec![
                SelfExpr::LetRec(
                    (),
                    LetRec {
                        bindings: defines,
                        body: exprs,
                    },
                )
                .with_span(span),
            ])
        }
    }
}
