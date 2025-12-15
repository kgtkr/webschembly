use webschembly_compiler_locate::{L, LocatedValue};

use webschembly_compiler_ast::*;
use webschembly_compiler_sexpr as sexpr;

pub trait DesugaredPrevPhase = AstPhase<XExt = !>;

#[derive(Debug, Clone)]
pub struct Desugared<P: DesugaredPrevPhase>(std::marker::PhantomData<P>);

impl<P: DesugaredPrevPhase> ExtendAstPhase for Desugared<P> {
    type Prev = P;
    type XBegin = !;
    type XQuote = !;
    type XLetStar = !;
    type XCond = !;
    type XNamedLet = !;
    type XIf = ();
    type XVar = ();
    type XLet = ();
    type XCall = ();
    type XConst = ();
    type XCons = ();
    type XVector = ();
    type XUVector = ();
    type XLambda = ();
    type XLetRec = ();
}

impl<P: DesugaredPrevPhase> Desugared<P> {
    pub fn from_ast(ast: Ast<P>) -> Ast<Self> {
        let mut var_counter = 0;
        Ast {
            x: ast.x,
            exprs: Self::from_exprs(ast.exprs, &mut var_counter),
        }
    }

    fn gen_temp_var(var_counter: &mut usize) -> String {
        let var_name = format!("__desugared_temp_{}", var_counter);
        *var_counter += 1;
        var_name
    }

    fn from_expr(expr: LExpr<P>, exprs: &mut Vec<LExpr<Self>>, var_counter: &mut usize) {
        match expr.value {
            Expr::Const(_, lit) => exprs.push(Expr::Const((), lit).with_span(expr.span)),
            Expr::Var(_, var) => exprs.push(Expr::Var((), var).with_span(expr.span)),
            Expr::Define(x, def) => exprs.push(
                Expr::Define(
                    x,
                    Define {
                        name: def.name,
                        expr: Self::from_exprs(def.expr, var_counter),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Lambda(_, lambda) => exprs.push(
                Expr::Lambda(
                    (),
                    Lambda {
                        args: lambda.args,
                        body: Self::from_exprs(lambda.body, var_counter),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::If(_, if_) => exprs.push(
                Expr::If(
                    (),
                    If {
                        cond: Self::from_exprs(if_.cond, var_counter),
                        then: Self::from_exprs(if_.then, var_counter),
                        els: Self::from_exprs(if_.els, var_counter),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Cond(_, cond) => {
                let mut else_branch = Vec::new();
                for clause in cond.clauses.into_iter().rev() {
                    match clause {
                        CondClause::Else { body } => {
                            else_branch = Self::from_exprs(body, var_counter);
                        }
                        CondClause::Test { test, body } => {
                            let then_branch = Self::from_exprs(body, var_counter);
                            let cond_branch = Self::from_exprs(test, var_counter);
                            else_branch = vec![
                                Expr::If(
                                    (),
                                    If {
                                        cond: cond_branch,
                                        then: then_branch,
                                        els: else_branch,
                                    },
                                )
                                .with_span(expr.span),
                            ];
                        }
                        CondClause::TestOnly { test } => {
                            let cond_branch = Self::from_exprs(test, var_counter);
                            let temp_var = Self::gen_temp_var(var_counter);
                            else_branch = vec![
                                Expr::Let(
                                    (),
                                    LetLike {
                                        bindings: vec![
                                            Binding {
                                                name: L {
                                                    span: expr.span,
                                                    value: temp_var.clone(),
                                                },
                                                expr: cond_branch,
                                            }
                                            .with_span(expr.span),
                                        ],
                                        body: vec![
                                            Expr::If(
                                                (),
                                                If {
                                                    cond: vec![
                                                        Expr::Var((), temp_var.clone())
                                                            .with_span(expr.span),
                                                    ],
                                                    then: vec![
                                                        Expr::Var((), temp_var.clone())
                                                            .with_span(expr.span),
                                                    ],
                                                    els: else_branch,
                                                },
                                            )
                                            .with_span(expr.span),
                                        ],
                                    },
                                )
                                .with_span(expr.span),
                            ];
                        }
                        CondClause::Allow { test, func } => {
                            let cond_branch = Self::from_exprs(test, var_counter);
                            let func_branch = Self::from_exprs(func, var_counter);
                            let temp_var = Self::gen_temp_var(var_counter);
                            else_branch = vec![
                                Expr::Let(
                                    (),
                                    LetLike {
                                        bindings: vec![
                                            Binding {
                                                name: L {
                                                    span: expr.span,
                                                    value: temp_var.clone(),
                                                },
                                                expr: cond_branch,
                                            }
                                            .with_span(expr.span),
                                        ],
                                        body: vec![
                                            Expr::If(
                                                (),
                                                If {
                                                    cond: vec![
                                                        Expr::Var((), temp_var.clone())
                                                            .with_span(expr.span),
                                                    ],
                                                    then: vec![
                                                        Expr::Call(
                                                            (),
                                                            Call {
                                                                func: func_branch,
                                                                args: vec![vec![
                                                                    Expr::Var((), temp_var.clone())
                                                                        .with_span(expr.span),
                                                                ]],
                                                            },
                                                        )
                                                        .with_span(expr.span),
                                                    ],
                                                    els: else_branch,
                                                },
                                            )
                                            .with_span(expr.span),
                                        ],
                                    },
                                )
                                .with_span(expr.span),
                            ];
                        }
                    }
                }
                exprs.extend(else_branch);
            }
            Expr::Call(_, call) => exprs.push(
                Expr::Call(
                    (),
                    Call {
                        func: Self::from_exprs(call.func, var_counter),
                        args: call
                            .args
                            .into_iter()
                            .map(|expr| Self::from_exprs(expr, var_counter))
                            .collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Begin(_, begin) => {
                for expr in begin.exprs {
                    Self::from_expr(expr, exprs, var_counter);
                }
            }
            Expr::Set(x, set) => exprs.push(
                Expr::Set(
                    x,
                    Set {
                        name: set.name,
                        expr: Self::from_exprs(set.expr, var_counter),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Let(_, let_like) => exprs.push(
                Expr::Let((), Self::from_let_like(let_like, var_counter)).with_span(expr.span),
            ),
            Expr::LetStar(_, let_like) => {
                // 空のlet*でもスコープを作るためにバインディングが空のletで囲む
                let mut body = Expr::Let(
                    (),
                    LetLike {
                        bindings: vec![],
                        body: Self::from_exprs(let_like.body, var_counter),
                    },
                )
                .with_span(expr.span);
                for binding in let_like.bindings.into_iter().rev() {
                    body = Expr::Let(
                        (),
                        LetLike {
                            bindings: vec![Self::from_binding(binding, var_counter)],
                            body: vec![body],
                        },
                    )
                    .with_span(expr.span);
                }
                exprs.push(body);
            }
            Expr::LetRec(_, let_like) => exprs.push(
                Expr::LetRec((), Self::from_let_like(let_like, var_counter)).with_span(expr.span),
            ),
            Expr::NamedLet(_, name, let_like) => {
                // from: (let tag ((name val) ...) body1 body2 ...)
                // to: (letrec ((tag (lambda (name ...) body1 body2 ...))) (tag val ...))
                let func_var = name.value.clone();
                let lambda = Expr::Lambda(
                    (),
                    Lambda {
                        args: let_like
                            .bindings
                            .iter()
                            .map(|b| b.value.name.clone())
                            .collect(),
                        body: Self::from_exprs(let_like.body, var_counter),
                    },
                )
                .with_span(expr.span);
                let letrec = Expr::LetRec(
                    (),
                    LetLike {
                        bindings: vec![
                            Binding {
                                name: L {
                                    span: expr.span,
                                    value: func_var.clone(),
                                },
                                expr: vec![lambda],
                            }
                            .with_span(expr.span),
                        ],
                        body: vec![
                            Expr::Call(
                                (),
                                Call {
                                    func: vec![
                                        Expr::Var((), func_var.clone()).with_span(expr.span),
                                    ],
                                    args: let_like
                                        .bindings
                                        .into_iter()
                                        .map(|b| Self::from_exprs(b.value.expr, var_counter))
                                        .collect(),
                                },
                            )
                            .with_span(expr.span),
                        ],
                    },
                )
                .with_span(expr.span);
                exprs.push(letrec);
            }
            Expr::Vector(_, vec) => exprs.push(
                Expr::Vector(
                    (),
                    vec.into_iter()
                        .map(|expr| Self::from_exprs(expr, var_counter))
                        .collect(),
                )
                .with_span(expr.span),
            ),
            Expr::UVector(_, uvec) => exprs.push(
                Expr::UVector(
                    (),
                    UVector {
                        kind: uvec.kind,
                        elements: uvec
                            .elements
                            .into_iter()
                            .map(|expr| Self::from_exprs(expr, var_counter))
                            .collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Quote(_, sexpr) => exprs.push(Self::from_quoted_sexpr(sexpr)),
            Expr::Cons(_, cons) => exprs.push(
                Expr::Cons(
                    (),
                    Cons {
                        car: Self::from_exprs(cons.car, var_counter),
                        cdr: Self::from_exprs(cons.cdr, var_counter),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Ext(x) => x,
        }
    }

    fn from_quoted_sexpr(sexpr: sexpr::LSExpr) -> LExpr<Self> {
        match sexpr.value {
            sexpr::SExpr::Bool(b) => Expr::Const((), Const::Bool(b)).with_span(sexpr.span),
            sexpr::SExpr::Int(i) => Expr::Const((), Const::Int(i)).with_span(sexpr.span),
            sexpr::SExpr::Float(f) => Expr::Const((), Const::Float(f)).with_span(sexpr.span),
            sexpr::SExpr::NaN => Expr::Const((), Const::NaN).with_span(sexpr.span),
            sexpr::SExpr::String(s) => Expr::Const((), Const::String(s)).with_span(sexpr.span),
            sexpr::SExpr::Char(c) => Expr::Const((), Const::Char(c)).with_span(sexpr.span),
            sexpr::SExpr::Symbol(s) => Expr::Const((), Const::Symbol(s)).with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExpr::Cons(cons) => Expr::Cons(
                (),
                Cons {
                    car: vec![Self::from_quoted_sexpr(cons.car)],
                    cdr: vec![Self::from_quoted_sexpr(cons.cdr)],
                },
            )
            .with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExpr::Vector(vec) => Expr::Vector(
                (),
                vec.into_iter()
                    .map(|s| vec![Self::from_quoted_sexpr(s)])
                    .collect(),
            )
            .with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExpr::UVector(kind, elements) => Expr::UVector(
                (),
                UVector {
                    kind: match kind {
                        sexpr::SUVectorKind::S64 => UVectorKind::S64,
                        sexpr::SUVectorKind::F64 => UVectorKind::F64,
                    },
                    elements: elements
                        .into_iter()
                        .map(|s| vec![Self::from_quoted_sexpr(s)])
                        .collect(),
                },
            )
            .with_span(sexpr.span),
            sexpr::SExpr::Nil => Expr::Const((), Const::Nil).with_span(sexpr.span),
        }
    }

    fn from_exprs(exprs: Vec<LExpr<P>>, var_counter: &mut usize) -> Vec<LExpr<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            Self::from_expr(expr, &mut result, var_counter);
        }
        result
    }

    fn from_let_like(let_like: LetLike<P>, var_counter: &mut usize) -> LetLike<Self> {
        LetLike {
            bindings: let_like
                .bindings
                .into_iter()
                .map(|binding| Self::from_binding(binding, var_counter))
                .collect(),
            body: Self::from_exprs(let_like.body, var_counter),
        }
    }

    fn from_binding(binding: L<Binding<P>>, var_counter: &mut usize) -> L<Binding<Self>> {
        Binding {
            name: binding.value.name,
            expr: Self::from_exprs(binding.value.expr, var_counter),
        }
        .with_span(binding.span)
    }
}
