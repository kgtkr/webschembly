use webschembly_compiler_locate::{L, LocatedValue};

use webschembly_compiler_ast::*;
use webschembly_compiler_sexpr as sexpr;

pub trait DesugaredPrevPhase = AstPhase<XExt = !>;

#[derive(Debug, Clone)]
pub struct Desugared<P: DesugaredPrevPhase> {
    _marker: std::marker::PhantomData<P>,
    var_counter: usize,
}

impl<P: DesugaredPrevPhase> ExtendAstPhase for Desugared<P> {
    type Prev = P;
    type XBegin = !;
    type XQuote = !;
    type XLetStar = !;
    type XCond = !;
    type XNamedLet = !;
    type XDo = !;
    type XAnd = !;
    type XOr = !;
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

impl<P: DesugaredPrevPhase> Default for Desugared<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: DesugaredPrevPhase> Desugared<P> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
            var_counter: 0,
        }
    }

    pub fn from_ast(&mut self, ast: Ast<P>) -> Ast<Self> {
        Ast {
            x: ast.x,
            exprs: self.process_exprs(ast.exprs),
        }
    }

    fn gen_temp_var(&mut self) -> String {
        let var_name = format!("__desugared_temp_{}", self.var_counter);
        self.var_counter += 1;
        var_name
    }

    fn process_expr(&mut self, expr: LExpr<P>, exprs: &mut Vec<LExpr<Self>>) {
        match expr.value {
            Expr::Const(_, lit) => exprs.push(Expr::Const((), lit).with_span(expr.span)),
            Expr::Var(_, var) => exprs.push(Expr::Var((), var).with_span(expr.span)),
            Expr::Define(x, def) => exprs.push(
                Expr::Define(
                    x,
                    Define {
                        name: def.name,
                        expr: self.process_exprs(def.expr),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Lambda(_, lambda) => exprs.push(
                Expr::Lambda(
                    (),
                    Lambda {
                        args: lambda.args,
                        variadic_arg: lambda.variadic_arg,
                        body: self.process_exprs(lambda.body),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::If(_, if_) => exprs.push(
                Expr::If(
                    (),
                    If {
                        cond: self.process_exprs(if_.cond),
                        then: self.process_exprs(if_.then),
                        els: self.process_exprs(if_.els),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Cond(_, cond) => {
                let mut else_branch = Vec::new();
                for clause in cond.clauses.into_iter().rev() {
                    match clause {
                        CondClause::Else { body } => {
                            else_branch = self.process_exprs(body);
                        }
                        CondClause::Test { test, body } => {
                            let then_branch = self.process_exprs(body);
                            let cond_branch = self.process_exprs(test);
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
                            let cond_branch = self.process_exprs(test);
                            let temp_var = self.gen_temp_var();
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
                            let cond_branch = self.process_exprs(test);
                            let func_branch = self.process_exprs(func);
                            let temp_var = self.gen_temp_var();
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
                        func: self.process_exprs(call.func),
                        args: call
                            .args
                            .into_iter()
                            .map(|expr| self.process_exprs(expr))
                            .collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Begin(_, begin) => {
                for expr in begin.exprs {
                    self.process_expr(expr, exprs);
                }
            }
            Expr::Set(x, set) => exprs.push(
                Expr::Set(
                    x,
                    Set {
                        name: set.name,
                        expr: self.process_exprs(set.expr),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Let(_, let_like) => {
                exprs.push(Expr::Let((), self.process_let_like(let_like)).with_span(expr.span))
            }
            Expr::LetStar(_, let_like) => {
                // 空のlet*でもスコープを作るためにバインディングが空のletで囲む
                let mut body = Expr::Let(
                    (),
                    LetLike {
                        bindings: vec![],
                        body: self.process_exprs(let_like.body),
                    },
                )
                .with_span(expr.span);
                for binding in let_like.bindings.into_iter().rev() {
                    body = Expr::Let(
                        (),
                        LetLike {
                            bindings: vec![self.process_binding(binding)],
                            body: vec![body],
                        },
                    )
                    .with_span(expr.span);
                }
                exprs.push(body);
            }
            Expr::LetRec(_, let_like) => {
                exprs.push(Expr::LetRec((), self.process_let_like(let_like)).with_span(expr.span))
            }
            Expr::NamedLet(_, name, let_like) => {
                let func_var = name.value.clone();
                let lambda = Expr::Lambda(
                    (),
                    Lambda {
                        args: let_like
                            .bindings
                            .iter()
                            .map(|b| b.value.name.clone())
                            .collect(),
                        variadic_arg: None,
                        body: self.process_exprs(let_like.body),
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
                                        .map(|b| self.process_exprs(b.value.expr))
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
            Expr::Do(_, do_) => {
                let loop_var = self.gen_temp_var();
                let letrec = Expr::LetRec(
                    (),
                    LetLike {
                        bindings: vec![
                            Binding {
                                name: L {
                                    span: expr.span,
                                    value: loop_var.clone(),
                                },
                                expr: vec![
                                    Expr::Lambda(
                                        (),
                                        Lambda {
                                            args: do_
                                                .bindings
                                                .iter()
                                                .map(|b| b.value.name.clone())
                                                .collect(),
                                            variadic_arg: None,
                                            body: vec![
                                                Expr::If(
                                                    (),
                                                    If {
                                                        cond: self.process_exprs(do_.test),
                                                        then: self.process_exprs(do_.exit_body),
                                                        els: {
                                                            let mut body = self.process_exprs(do_.body);
                                                            body.push(
                                                                Expr::Call(
                                                                    (),
                                                                    Call {
                                                                        func: vec![
                                                                            Expr::Var((), loop_var.clone())
                                                                                .with_span(expr.span),
                                                                        ],
                                                                        args: do_
                                                                            .bindings
                                                                            .iter()
                                                                            .map(|b| {
                                                                                 b.value.step.clone().map(|step|self.process_exprs(step))
                                                                                    .unwrap_or_else(|| {
                                                                                        vec![
                                                                                            Expr::Var((), b.value.name.value.clone())
                                                                                                .with_span(b.span),
                                                                                        ]
                                                                                    })
                                                                            })
                                                                            .collect(),
                                                                    },
                                                                )
                                                                .with_span(expr.span),
                                                            );
                                                            body
                                                        },
                                                    },
                                                )
                                                .with_span(expr.span),
                                            ],
                                        },
                                    )
                                    .with_span(expr.span),
                                ],
                            }
                            .with_span(expr.span),
                        ],
                        body: vec![
                            Expr::Call(
                                (),
                                Call {
                                    func: vec![Expr::Var((), loop_var.clone()).with_span(expr.span)],
                                    args: do_
                                        .bindings
                                        .into_iter()
                                        .map(|b| self.process_exprs(b.value.init))
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
            Expr::And(_, and_exprs) => {
                if and_exprs.is_empty() {
                    exprs.push(Expr::Const((), Const::Bool(true)).with_span(expr.span));
                } else if and_exprs.len() == 1 {
                    exprs.extend(self.process_exprs(and_exprs.into_iter().next().unwrap()));
                } else {
                    let mut iter = and_exprs.into_iter().rev();
                    let mut else_branch = self.process_exprs(iter.next().unwrap());
                    for and_expr in iter {
                        let cond = self.process_exprs(and_expr);
                        else_branch = vec![
                            Expr::If(
                                (),
                                If {
                                    cond,
                                    then: else_branch,
                                    els: vec![
                                        Expr::Const((), Const::Bool(false)).with_span(expr.span),
                                    ],
                                },
                            )
                            .with_span(expr.span),
                        ];
                    }
                    exprs.extend(else_branch);
                }
            }
            Expr::Or(_, or_exprs) => {
                if or_exprs.is_empty() {
                    exprs.push(Expr::Const((), Const::Bool(false)).with_span(expr.span));
                } else if or_exprs.len() == 1 {
                    exprs.extend(self.process_exprs(or_exprs.into_iter().next().unwrap()));
                } else {
                    let mut else_branch =
                        vec![Expr::Const((), Const::Bool(false)).with_span(expr.span)];
                    for or_expr in or_exprs.into_iter().rev() {
                        let test = self.process_exprs(or_expr);
                        let temp_var = self.gen_temp_var();
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
                                            expr: test,
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
                    exprs.extend(else_branch);
                }
            }
            Expr::Vector(_, vec) => exprs.push(
                Expr::Vector(
                    (),
                    vec.into_iter()
                        .map(|expr| self.process_exprs(expr))
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
                            .map(|expr| self.process_exprs(expr))
                            .collect(),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Quote(_, sexpr) => exprs.push(Self::process_quoted_sexpr(sexpr)),
            Expr::Cons(_, cons) => exprs.push(
                Expr::Cons(
                    (),
                    Cons {
                        car: self.process_exprs(cons.car),
                        cdr: self.process_exprs(cons.cdr),
                    },
                )
                .with_span(expr.span),
            ),
            Expr::Ext(x) => x,
        }
    }

    fn process_quoted_sexpr(sexpr: sexpr::LSExpr) -> LExpr<Self> {
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
                    car: vec![Self::process_quoted_sexpr(cons.car)],
                    cdr: vec![Self::process_quoted_sexpr(cons.cdr)],
                },
            )
            .with_span(sexpr.span),
            // TODO: span情報の保持
            sexpr::SExpr::Vector(vec) => Expr::Vector(
                (),
                vec.into_iter()
                    .map(|s| vec![Self::process_quoted_sexpr(s)])
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
                        .map(|s| vec![Self::process_quoted_sexpr(s)])
                        .collect(),
                },
            )
            .with_span(sexpr.span),
            sexpr::SExpr::Nil => Expr::Const((), Const::Nil).with_span(sexpr.span),
        }
    }

    fn process_exprs(&mut self, exprs: Vec<LExpr<P>>) -> Vec<LExpr<Self>> {
        let mut result = Vec::new();
        for expr in exprs {
            self.process_expr(expr, &mut result);
        }
        result
    }

    fn process_let_like(&mut self, let_like: LetLike<P>) -> LetLike<Self> {
        LetLike {
            bindings: let_like
                .bindings
                .into_iter()
                .map(|binding| self.process_binding(binding))
                .collect(),
            body: self.process_exprs(let_like.body),
        }
    }

    fn process_binding(&mut self, binding: L<Binding<P>>) -> L<Binding<Self>> {
        Binding {
            name: binding.value.name,
            expr: self.process_exprs(binding.value.expr),
        }
        .with_span(binding.span)
    }
}
