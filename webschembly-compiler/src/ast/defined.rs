use super::Desugared;
use super::astx::*;
use crate::ast::parsed;
use crate::compiler_error;
use crate::error::Result;
use crate::x::FamilyX;
use crate::x::Phase;
use crate::x::TypeMap;
use crate::x::type_map;
use crate::x::type_map::ElementInto;
use crate::x::type_map::IntoTypeMap;

// 変数の巻き上げを行うためにラムダ式で定義されている変数の名前リストを作成する
// また、変数の重複チェックと、defineできない場所でdefineが行われていないかも確認する

#[derive(Debug, Clone)]
pub enum Defined {}

impl Phase for Defined {
    type Prev = Desugared;
}

#[derive(Debug, Clone)]
pub struct DefinedLambdaR {
    pub defines: Vec<String>,
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
    type R = DefinedLambdaR;
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
    type R = DefinedSetR;
}

#[derive(Debug, Clone)]
pub struct DefinedSetR {
    pub reassign: bool,
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

#[derive(Debug, Clone)]
pub struct DefinedLetR {
    pub defines: Vec<String>,
}

impl FamilyX<Defined> for LetX {
    type R = DefinedLetR;
}

#[derive(Debug, Clone)]
pub struct DefinedLetRecR {
    pub defines: Vec<String>,
}

impl FamilyX<Defined> for LetRecX {
    type R = DefinedLetRecR;
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
        names: &mut Vec<String>,
    ) -> Result<(DefineContext, Self)> {
        match expr {
            Expr::Const(x, lit) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::Const(x.add(type_map::key::<Defined>(), ()), lit),
            )),
            Expr::Var(x, var) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::Var(x.add(type_map::key::<Defined>(), ()), var),
            )),
            Expr::Define(x, def) => {
                match ctx {
                    DefineContext::Global => {}
                    DefineContext::LocalDefinable => {
                        if names.contains(&def.name) {
                            return Err(compiler_error!(
                                "Variable {} is already defined",
                                def.name
                            ));
                        } else {
                            names.push(def.name.clone());
                        }
                    }
                    DefineContext::LocalUndefinable => {
                        return Err(compiler_error!(
                            "Define is not allowed in this context: {}",
                            def.name
                        ));
                    }
                };

                // defineは巻き上げを行う以外set!と同じ
                Ok((
                    ctx,
                    Expr::Set(
                        // defineは再帰呼び出し可能なのでreassign: trueにする必要がある
                        x.into_type_map(())
                            .add(type_map::key::<Defined>(), DefinedSetR { reassign: true }),
                        Set {
                            name: def.name,
                            expr: Self::from_exprs(def.expr, ctx.to_undefinable_if_local(), names)?,
                        },
                    ),
                ))
            }
            Expr::Lambda(x, lambda) => {
                let mut names = Vec::new();
                let new_body =
                    Self::from_exprs(lambda.body, DefineContext::LocalDefinable, &mut names)?;
                Ok((
                    ctx,
                    Expr::Lambda(
                        x.add(
                            type_map::key::<Defined>(),
                            DefinedLambdaR { defines: names },
                        ),
                        Lambda {
                            args: lambda.args,
                            body: new_body,
                        },
                    ),
                ))
            }
            Expr::If(x, if_) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::If(
                    x.add(type_map::key::<Defined>(), ()),
                    If {
                        cond: Self::from_exprs(if_.cond, ctx.to_undefinable_if_local(), names)?,
                        then: Self::from_exprs(if_.then, ctx.to_undefinable_if_local(), names)?,
                        els: Self::from_exprs(if_.els, ctx.to_undefinable_if_local(), names)?,
                    },
                ),
            )),
            Expr::Call(x, call) => {
                let new_func = Self::from_exprs(call.func, ctx.to_undefinable_if_local(), names)?;
                let new_args = call
                    .args
                    .into_iter()
                    .map(|arg| Self::from_exprs(arg, ctx.to_undefinable_if_local(), names))
                    .collect::<Result<Vec<_>>>()?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Call(
                        x.add(type_map::key::<Defined>(), ()),
                        Call {
                            func: new_func,
                            args: new_args,
                        },
                    ),
                ))
            }
            Expr::Begin(x, begin) => {
                let new_exprs = Self::from_exprs(begin.exprs, ctx, names)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Begin(
                        x.add(type_map::key::<Defined>(), ()),
                        Begin { exprs: new_exprs },
                    ),
                ))
            }
            Expr::Set(x, set) => {
                let new_expr = Self::from_exprs(set.expr, ctx.to_undefinable_if_local(), names)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Set(
                        x.add(type_map::key::<Defined>(), DefinedSetR { reassign: true }),
                        Set {
                            name: set.name,
                            expr: new_expr,
                        },
                    ),
                ))
            }
            Expr::Let(x, let_) => {
                let mut new_names = Vec::new();
                let new_body =
                    Self::from_exprs(let_.body, DefineContext::LocalDefinable, &mut new_names)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::Let(
                        x.add(
                            type_map::key::<Defined>(),
                            DefinedLetR { defines: new_names },
                        ),
                        Let {
                            bindings: let_
                                .bindings
                                .into_iter()
                                .map(|(name, expr)| {
                                    Self::from_exprs(expr, ctx.to_undefinable_if_local(), names)
                                        .map(|expr| (name, expr))
                                })
                                .collect::<Result<Vec<_>>>()?,
                            body: new_body,
                        },
                    ),
                ))
            }
            Expr::LetRec(x, letrec) => {
                let mut new_names = Vec::new();
                let new_body =
                    Self::from_exprs(letrec.body, DefineContext::LocalDefinable, &mut new_names)?;
                Ok((
                    ctx.to_undefinable_if_local(),
                    Expr::LetRec(
                        x.add(
                            type_map::key::<Defined>(),
                            DefinedLetRecR { defines: new_names },
                        ),
                        LetRec {
                            bindings: letrec
                                .bindings
                                .into_iter()
                                .map(|(name, expr)| {
                                    Self::from_exprs(expr, ctx.to_undefinable_if_local(), names)
                                        .map(|expr| (name, expr))
                                })
                                .collect::<Result<Vec<_>>>()?,
                            body: new_body,
                        },
                    ),
                ))
            }
            Expr::Vector(x, vec) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::Vector(
                    x.add(type_map::key::<Defined>(), ()),
                    vec.into_iter()
                        .map(|v| {
                            Self::from_expr(v, ctx.to_undefinable_if_local(), names)
                                .map(|(_, expr)| expr)
                        })
                        .collect::<Result<Vec<_>>>()?,
                ),
            )),
            Expr::UVector(x, uvec) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::UVector(
                    x.add(type_map::key::<Defined>(), ()),
                    UVector {
                        kind: uvec.kind,
                        elements: uvec
                            .elements
                            .into_iter()
                            .map(|v| {
                                Self::from_expr(v, ctx.to_undefinable_if_local(), names)
                                    .map(|(_, expr)| expr)
                            })
                            .collect::<Result<Vec<_>>>()?,
                    },
                ),
            )),
            Expr::Quote(x, _) => x.get_owned(type_map::key::<Desugared>()),
            Expr::Cons(x, cons) => Ok((
                ctx.to_undefinable_if_local(),
                Expr::Cons(
                    x.add(type_map::key::<Defined>(), ()),
                    Cons {
                        car: Self::from_exprs(cons.car, ctx.to_undefinable_if_local(), names)?,
                        cdr: Self::from_exprs(cons.cdr, ctx.to_undefinable_if_local(), names)?,
                    },
                ),
            )),
        }
    }

    fn from_exprs(
        exprs: Vec<Expr<<Defined as Phase>::Prev>>,
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
