use webschembly_compiler_ast::*;
use webschembly_compiler_locate::LocatedValue;

#[derive(Debug, Clone)]
pub struct TailCall<P: TailCallPrevPhase>(std::marker::PhantomData<P>);

impl<P: TailCallPrevPhase> ExtendAstPhase for TailCall<P> {
    type Prev = P;
    type XCall = TailCallCallR;
}

#[derive(Debug, Clone)]
pub struct TailCallCallR {
    pub is_tail: bool,
}

pub trait TailCallPrevPhase = AstPhase<
        XBegin = !,
        XQuote = !,
        XDefine = !,
        XLetStar = !,
        XExt = !,
        XCond = !,
        XNamedLet = !,
        XDo = !,
        XAnd = !,
        XOr = !,
    >;

impl<P: TailCallPrevPhase> TailCall<P> {
    pub fn from_ast(ast: Ast<P>) -> Ast<Self> {
        Ast {
            x: ast.x,
            exprs: ast
                .exprs
                .into_iter()
                .map(|expr| Self::from_expr(expr, false))
                .collect(),
        }
    }

    fn from_expr(expr: LExpr<P>, is_tail: bool) -> LExpr<Self> {
        match expr.value {
            Expr::Const(x, lit) => Expr::Const(x, lit).with_span(expr.span),
            Expr::Var(x, var) => Expr::Var(x, var).with_span(expr.span),
            Expr::Define(x, _) => x,
            Expr::Lambda(x, lambda) => Expr::Lambda(
                x,
                Lambda {
                    args: lambda.args,
                    variadic_arg: lambda.variadic_arg,
                    body: Self::from_exprs(lambda.body, true),
                },
            )
            .with_span(expr.span),
            Expr::If(x, if_) => Expr::If(
                x,
                If {
                    cond: Self::from_exprs(if_.cond, false),
                    then: Self::from_exprs(if_.then, is_tail),
                    els: Self::from_exprs(if_.els, is_tail),
                },
            )
            .with_span(expr.span),
            Expr::Cond(x, _) => x,
            Expr::Call(_, call) => Expr::Call(
                TailCallCallR { is_tail },
                Call {
                    func: Self::from_exprs(call.func, false),
                    args: call
                        .args
                        .into_iter()
                        .map(|arg| Self::from_exprs(arg, false))
                        .collect(),
                },
            )
            .with_span(expr.span),
            Expr::Begin(x, _) => x,
            Expr::Set(x, set) => Expr::Set(
                x,
                Set {
                    name: set.name,
                    expr: Self::from_exprs(set.expr, false),
                },
            )
            .with_span(expr.span),
            Expr::Let(x, let_like) => {
                Expr::Let(x, Self::from_let_like(let_like, is_tail)).with_span(expr.span)
            }
            Expr::LetStar(x, _) => x,
            Expr::LetRec(x, let_like) => {
                Expr::LetRec(x, Self::from_let_like(let_like, is_tail)).with_span(expr.span)
            }
            Expr::Vector(x, vec) => Expr::Vector(x, {
                vec.into_iter()
                    .map(|expr| Self::from_exprs(expr, false))
                    .collect()
            })
            .with_span(expr.span),
            Expr::UVector(x, uvec) => Expr::UVector(
                x,
                UVector {
                    kind: uvec.kind,
                    elements: uvec
                        .elements
                        .into_iter()
                        .map(|expr| Self::from_exprs(expr, false))
                        .collect(),
                },
            )
            .with_span(expr.span),
            Expr::Quote(x, _) => x,
            Expr::Cons(x, cons) => Expr::Cons(
                x,
                Cons {
                    car: Self::from_exprs(cons.car, false),
                    cdr: Self::from_exprs(cons.cdr, false),
                },
            )
            .with_span(expr.span),
            Expr::Ext(x) => x,
        }
    }

    fn from_exprs(exprs: Vec<LExpr<P>>, is_tail: bool) -> Vec<LExpr<Self>> {
        let n = exprs.len();
        exprs
            .into_iter()
            .enumerate()
            .map(|(i, expr)| Self::from_expr(expr, is_tail && i == n - 1))
            .collect()
    }

    fn from_let_like(let_like: LetLike<P>, is_tail: bool) -> LetLike<Self> {
        LetLike {
            bindings: let_like
                .bindings
                .into_iter()
                .map(|binding| {
                    Binding {
                        name: binding.value.name,
                        expr: Self::from_exprs(binding.value.expr, false),
                    }
                    .with_span(binding.span)
                })
                .collect(),
            body: Self::from_exprs(let_like.body, is_tail),
        }
    }
}
