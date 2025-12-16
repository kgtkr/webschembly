use webschembly_compiler_ast::*;
use webschembly_compiler_error::{Result, compiler_error};
use webschembly_compiler_locate::{Located, LocatedValue, Span};
// defineをletrec or set!に変換

pub trait DefinedPrevPhase =
    AstPhase<XBegin = !, XQuote = !, XLetStar = !, XCond = !, XExt = !, XNamedLet = !, XDo = !>;

#[derive(Debug, Clone)]
pub struct Defined<P: DefinedPrevPhase>(std::marker::PhantomData<P>);

impl<P: DefinedPrevPhase> ExtendAstPhase for Defined<P> {
    type Prev = P;
    type XDefine = !;
    type XSet = ();
    type XLetRec = ();
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

impl<P: DefinedPrevPhase> Defined<P> {
    pub fn from_ast(ast: Ast<P>) -> Result<Ast<Self>> {
        let new_exprs = Self::from_exprs(ast.exprs, DefineContext::Global, &mut Vec::new())?;
        Ok(Ast {
            x: ast.x,
            exprs: new_exprs,
        })
    }

    fn from_expr(
        expr: LExpr<P>,
        ctx: DefineContext,
        defines: &mut Vec<Located<Binding<Self>>>,
        result: &mut Vec<LExpr<Self>>,
    ) -> Result<()> {
        match expr.value {
            Expr::Const(x, lit) => {
                result.push(Expr::Const(x, lit).with_span(expr.span));
                Ok(())
            }
            Expr::Var(x, var) => {
                result.push(Expr::Var(x, var).with_span(expr.span));
                Ok(())
            }
            Expr::Define(_, def) => {
                match ctx {
                    DefineContext::Global => {
                        result.push(
                            Expr::Set(
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
            Expr::Lambda(x, lambda) => {
                let new_body = Self::from_exprs_new_scope(expr.span, lambda.body)?;
                result.push(
                    Expr::Lambda(
                        x,
                        Lambda {
                            args: lambda.args,
                            variadic_arg: lambda.variadic_arg,
                            body: new_body,
                        },
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::If(x, if_) => {
                result.push(
                    Expr::If(
                        x,
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
            Expr::Cond(x, _) => x,
            Expr::Call(x, call) => {
                let new_func = Self::from_exprs(call.func, ctx.to_undefinable_if_local(), defines)?;
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_exprs(arg, ctx.to_undefinable_if_local(), defines))
                    .collect::<Result<Vec<_>>>()?;
                result.push(
                    Expr::Call(
                        x,
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
                    Expr::Set(
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
            Expr::Let(x, let_like) => {
                result.push(
                    Expr::Let(x, Self::from_let_like(let_like, expr.span, ctx, defines)?)
                        .with_span(expr.span),
                );
                Ok(())
            }
            Expr::LetStar(x, _) => x,
            Expr::LetRec(_, let_like) => {
                result.push(
                    Expr::LetRec((), Self::from_let_like(let_like, expr.span, ctx, defines)?)
                        .with_span(expr.span),
                );
                Ok(())
            }

            Expr::Vector(x, vec) => {
                result.push(
                    Expr::Vector(
                        x,
                        vec.into_iter()
                            .map(|v| Self::from_exprs(v, ctx.to_undefinable_if_local(), defines))
                            .collect::<Result<Vec<_>>>()?,
                    )
                    .with_span(expr.span),
                );
                Ok(())
            }
            Expr::UVector(x, uvec) => {
                result.push(
                    Expr::UVector(
                        x,
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
            Expr::Cons(x, cons) => {
                result.push(
                    Expr::Cons(
                        x,
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
            Expr::Ext(x) => x,
        }
    }

    fn from_exprs(
        exprs: Vec<LExpr<P>>,
        mut ctx: DefineContext,
        defines: &mut Vec<Located<Binding<Self>>>,
    ) -> Result<Vec<LExpr<Self>>> {
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
    fn from_exprs_new_scope(span: Span, exprs: Vec<LExpr<P>>) -> Result<Vec<LExpr<Self>>> {
        let mut defines = Vec::new();
        let exprs = Self::from_exprs(exprs, DefineContext::LocalDefinable, &mut defines)?;
        if defines.is_empty() {
            Ok(exprs)
        } else {
            Ok(vec![
                Expr::LetRec(
                    (),
                    LetLike {
                        bindings: defines,
                        body: exprs,
                    },
                )
                .with_span(span),
            ])
        }
    }

    fn from_let_like(
        let_like: LetLike<P>,
        span: Span,
        ctx: DefineContext,
        defines: &mut Vec<Located<Binding<Self>>>,
    ) -> Result<LetLike<Self>> {
        let new_body = Self::from_exprs_new_scope(span, let_like.body)?;
        Ok(LetLike {
            bindings: let_like
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
        })
    }
}
