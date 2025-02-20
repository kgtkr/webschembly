use super::ast::*;
use super::parsed::*;
use crate::x::FamilyX;
use anyhow::Result;

// 変数の巻き上げを行うためにラムダ式で定義されている変数の名前リストを作成する
// また、変数の重複チェックと、defineできない場所でdefineが行われていないかも確認する

#[derive(Debug, Clone)]
pub enum Defined {}

type Prev = Parsed;

#[derive(Debug, Clone)]
pub struct DefinedLambdaR {
    pub defines: Vec<String>,
}

impl FamilyX<Defined> for AstX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for LiteralX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for DefineX {
    type R = !;
}
impl FamilyX<Defined> for LambdaX {
    type R = DefinedLambdaR;
}
impl FamilyX<Defined> for IfX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for CallX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for VarX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for BeginX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for DumpX {
    type R = <Self as FamilyX<Prev>>::R;
}
impl FamilyX<Defined> for SetX {
    type R = <Self as FamilyX<Prev>>::R;
}

impl Ast<Defined> {
    pub fn from_ast(ast: Ast<Prev>) -> Result<Self> {
        let new_exprs: Vec<Expr<Defined>> =
            Expr::<Defined>::from_block(ast.exprs, DefineContext::Global, &mut Vec::new())?;
        Ok(Ast {
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

impl Expr<Defined> {
    fn from_expr(
        expr: Expr<Prev>,
        ctx: DefineContext,
        names: &mut Vec<String>,
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
                            names.push(def.name.clone());
                        }
                    }
                    DefineContext::LocalUndefinable => {
                        return Err(anyhow::anyhow!(
                            "Define is not allowed in this context: {}",
                            def.name
                        ))
                    }
                };

                // defineは巻き上げを行う以外set!と同じ
                Ok((
                    ctx,
                    Expr::Set(
                        x,
                        Set {
                            name: def.name,
                            expr: Box::new(
                                Self::from_expr(*def.expr, ctx.to_undefinable_if_local(), names)
                                    .map(|(_, expr)| expr)?,
                            ),
                        },
                    ),
                ))
            }
            Expr::Lambda(_, lambda) => {
                let mut names = Vec::new();
                let new_body =
                    Self::from_block(lambda.body, DefineContext::LocalDefinable, &mut names)?;
                Ok((
                    ctx,
                    Expr::Lambda(
                        DefinedLambdaR { defines: names },
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
                            Self::from_expr(*if_.cond, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)?,
                        ),
                        then: Box::new(
                            Self::from_expr(*if_.then, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)?,
                        ),
                        els: Box::new(
                            Self::from_expr(*if_.els, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)?,
                        ),
                    },
                ),
            )),
            Expr::Call(_, call) => {
                let new_func = Self::from_expr(*call.func, ctx.to_undefinable_if_local(), names)
                    .map(|(_, expr)| expr)?;
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| {
                        Self::from_expr(arg, ctx.to_undefinable_if_local(), names)
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
                let new_expr = Self::from_expr(*dump, ctx.to_undefinable_if_local(), names)
                    .map(|(_, expr)| expr)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Dump((), Box::new(new_expr)),
                ))
            }
            Expr::Set(_, set) => {
                let new_expr = Self::from_expr(*set.expr, ctx.to_undefinable_if_local(), names)
                    .map(|(_, expr)| expr)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Set(
                        (),
                        Set {
                            name: set.name,
                            expr: Box::new(new_expr),
                        },
                    ),
                ))
            }
        }
    }

    fn from_block(
        exprs: Vec<Expr<Prev>>,
        mut ctx: DefineContext,
        names: &mut Vec<String>,
    ) -> Result<Vec<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            let (new_ctx, expr) = Self::from_expr(expr, ctx, names)?;
            ctx = new_ctx;
            result.push(expr);
        }
        Ok(result)
    }
}
