use std::collections::HashSet;

use super::ast::*;
use super::parsed::*;
use crate::x::FamilyX;
use anyhow::Result;

// 変数の巻き上げを行うためにラムダ式で定義されている変数の名前リストを作成する
// また、変数の重複チェックと、defineできない場所でdefineが行われていないかも確認する

#[derive(Debug, Clone, Copy)]
pub struct Defined;

pub type DefinedAST = AST<Defined>;
pub type DefinedExpr = Expr<Defined>;

#[derive(Debug, Clone)]
pub struct LambdaR {
    pub defines: HashSet<String>,
}

impl FamilyX<Defined> for AstX {
    type R = ();
}
impl FamilyX<Defined> for LiteralX {
    type R = ();
}
impl FamilyX<Defined> for DefineX {
    type R = ();
}
impl FamilyX<Defined> for LambdaX {
    type R = LambdaR;
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
impl FamilyX<Defined> for DumpX {
    type R = ();
}

impl DefinedAST {
    pub fn from_ast(ast: ParsedAST) -> Result<Self> {
        let new_exprs =
            DefinedExpr::from_block(ast.exprs, DefineContext::Global, &mut HashSet::new())?;
        Ok(DefinedAST {
            x: ast.x,
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

impl DefinedExpr {
    fn from_expr(
        expr: ParsedExpr,
        ctx: DefineContext,
        names: &mut HashSet<String>,
    ) -> Result<(DefineContext, Self)> {
        match expr {
            Expr::Literal(x, lit) => Ok((ctx.to_undefinable_if_local(), Expr::Literal(x, lit))),
            Expr::Var(x, var) => Ok((ctx.to_undefinable_if_local(), Expr::Var(x, var))),
            Expr::Define(x, def) => {
                match ctx {
                    DefineContext::Global => {}
                    DefineContext::LocalDefinable => {
                        if names.contains(&def.name) {
                            return Err(anyhow::anyhow!(
                                "Variable {} is already defined",
                                def.name
                            ));
                        } else {
                            names.insert(def.name.clone());
                        }
                    }
                    DefineContext::LocalUndefinable => {
                        return Err(anyhow::anyhow!(
                            "Define is not allowed in this context: {}",
                            def.name
                        ))
                    }
                };

                Ok((
                    ctx,
                    Expr::Define(
                        x,
                        Define {
                            name: def.name,
                            expr: Box::new(
                                DefinedExpr::from_expr(
                                    *def.expr,
                                    ctx.to_undefinable_if_local(),
                                    names,
                                )
                                .map(|(_, expr)| expr)?,
                            ),
                        },
                    ),
                ))
            }
            Expr::Lambda(x, lambda) => {
                let mut names = HashSet::new();
                let new_body =
                    Self::from_block(lambda.body, DefineContext::LocalDefinable, &mut names)?;
                Ok((
                    ctx,
                    Expr::Lambda(
                        LambdaR { defines: names },
                        Lambda {
                            args: lambda.args,
                            body: new_body,
                        },
                    ),
                ))
            }
            Expr::If(_, if_) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::If(
                    (),
                    If {
                        cond: Box::new(
                            DefinedExpr::from_expr(*if_.cond, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)?,
                        ),
                        then: Box::new(
                            DefinedExpr::from_expr(*if_.then, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)?,
                        ),
                        els: Box::new(
                            DefinedExpr::from_expr(*if_.els, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)?,
                        ),
                    },
                ),
            )),
            Expr::Call(_, call) => {
                let new_func =
                    DefinedExpr::from_expr(*call.func, ctx.to_undefinable_if_local(), names)
                        .map(|(_, expr)| expr)?;
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| {
                        DefinedExpr::from_expr(arg, ctx.to_undefinable_if_local(), names)
                            .map(|(_, expr)| expr)
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Call(
                        (),
                        Call {
                            func: Box::new(new_func),
                            args: new_args,
                        },
                    ),
                ))
            }
            Expr::Begin(_, begin) => {
                let new_exprs = Self::from_block(begin.exprs, ctx, names)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Begin((), Begin { exprs: new_exprs }),
                ))
            }
            Expr::Dump(_, dump) => {
                let new_expr = DefinedExpr::from_expr(*dump, ctx.to_undefinable_if_local(), names)
                    .map(|(_, expr)| expr)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Dump((), Box::new(new_expr)),
                ))
            }
        }
    }

    fn from_block(
        exprs: Vec<ParsedExpr>,
        mut ctx: DefineContext,
        names: &mut HashSet<String>,
    ) -> Result<Vec<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            let (new_ctx, expr) = DefinedExpr::from_expr(expr, ctx, names)?;
            ctx = new_ctx;
            result.push(expr);
        }
        Ok(result)
    }
}
