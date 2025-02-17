use std::collections::HashMap;

use crate::{ast, sexpr};
use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub enum Type {
    Boxed,
    Bool,
    Int,
    String,
    Symbol,
    Nil,
    Cons,
    Closure,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i32),
    String(String),
    StringToSymbol(usize),
    Nil,
    Cons(usize, usize),
    Closure(Vec<usize>, usize),
    /*
    set! が未実装なので一旦実装しない
    MutCell(usize),
    MutCellDeref(usize),*/
    Call(usize, Vec<usize>),
    Move(usize),
    BoxBool(usize),
    BoxInt(usize),
    BoxString(usize),
    BoxSymbol(usize),
    BoxNil(usize),
    BoxCons(usize),
    BoxClosure(usize),
    UnboxBool(usize),
    UnboxClosure(usize),
    Dump(usize),
    ClosureEnv(usize /* closure */, usize /* env index */),
}

#[derive(Debug, Clone)]
pub enum Stat {
    If(usize, Box<Stat>, Box<Stat>),
    Begin(Vec<Stat>),
    Expr(Option<usize>, Expr),
}

#[derive(Debug, Clone)]
pub struct Func {
    pub args: usize,
    pub rets: Vec<usize>,
    // argsを含む
    pub locals: Vec<Type>,
    pub body: Stat,
}

#[derive(Debug, Clone)]
pub struct Ir {
    pub funcs: Vec<Func>,
    pub entry: usize,
}

impl Ir {
    pub fn from_ast(ast: &ast::AST) -> Result<Ir> {
        let ir_gen = IrGenerator::new();

        Ok(ir_gen.gen(ast)?)
    }
}

#[derive(Debug, Clone)]
struct IrGenerator {
    funcs: Vec<Func>,
}

impl IrGenerator {
    fn new() -> Self {
        Self { funcs: Vec::new() }
    }

    fn gen(mut self, ast: &ast::AST) -> Result<Ir> {
        let lambda_gen = LambdaGenerator::new();
        let entry = lambda_gen.gen(
            &mut self,
            vec![],
            &ast::Lambda {
                args: Vec::new(),
                body: Box::new(ast.clone()),
            },
        )?;
        let entry_wrapper = self.funcs.len();
        self.funcs.push(Func {
            args: 0,
            rets: vec![],
            locals: vec![Type::Closure],
            body: Stat::Begin(vec![
                Stat::Expr(Some(0), Expr::Closure(vec![], entry)),
                Stat::Expr(None, Expr::Call(0, vec![0])),
            ]),
        });

        Ok(Ir {
            funcs: self.funcs,
            entry: entry_wrapper,
        })
    }
}

#[derive(Debug, Clone)]
struct LambdaGenerator {
    locals: Vec<Type>,
    local_names: HashMap<String, usize>,
}

impl LambdaGenerator {
    fn new() -> Self {
        Self {
            locals: Vec::new(),
            local_names: HashMap::new(),
        }
    }

    fn gen(
        mut self,
        ir_generator: &mut IrGenerator,
        envs: Vec<String>,
        lambda: &ast::Lambda,
    ) -> Result<usize> {
        let self_closure = self.local(Type::Closure);
        for arg in &lambda.args {
            self.named_local(arg.clone());
        }
        let mut stats = vec![];
        // クロージャから環境を復元
        for (i, env) in envs.iter().enumerate() {
            let env_local = self.named_local(env.clone());
            stats.push(Stat::Expr(
                Some(env_local),
                Expr::ClosureEnv(self_closure, i),
            ));
        }

        let ret = self.local(Type::Boxed);
        stats.push(self.gen_stat(ir_generator, Some(ret), &*lambda.body)?);
        let func = Func {
            args: lambda.args.len() + 1,
            rets: vec![ret],
            locals: self.locals,
            body: Stat::Begin(stats),
        };

        let func_id = ir_generator.funcs.len();
        ir_generator.funcs.push(func);
        Ok(func_id)
    }

    fn gen_stat(
        &mut self,
        ir_generator: &mut IrGenerator,
        result: Option<usize>,
        ast: &ast::AST,
    ) -> Result<Stat> {
        match ast {
            ast::AST::Bool(b) => {
                let unboxed = self.local(Type::Bool);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Bool(*b)),
                    Stat::Expr(result, Expr::BoxBool(unboxed)),
                ]))
            }
            ast::AST::Int(i) => {
                let unboxed = self.local(Type::Int);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Int(*i)),
                    Stat::Expr(result, Expr::BoxInt(unboxed)),
                ]))
            }
            ast::AST::String(s) => {
                let unboxed = self.local(Type::String);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::String(s.clone())),
                    Stat::Expr(result, Expr::BoxString(unboxed)),
                ]))
            }
            ast::AST::Nil => {
                let unboxed = self.local(Type::Nil);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Nil),
                    Stat::Expr(result, Expr::BoxNil(unboxed)),
                ]))
            }
            ast::AST::Quote(sexpr) => Ok(self.quote(result, sexpr)?),
            ast::AST::Define(name, expr) => {
                let local = self.named_local(name.clone());
                let stat = self.gen_stat(ir_generator, Some(local), expr)?;
                Ok(stat)
            }
            ast::AST::Lambda(lambda) => {
                // クロージャで使われているかに関わらず全てのローカル変数を環境に含める
                let local_names = self.local_names.iter().collect::<Vec<_>>();
                let names = local_names
                    .iter()
                    .map(|&(name, _)| name.clone())
                    .collect::<Vec<_>>();
                let ids = local_names
                    .iter()
                    .map(|&(_, &local)| local)
                    .collect::<Vec<_>>();
                let lambda_gen = LambdaGenerator::new();
                let func_id = lambda_gen.gen(ir_generator, names, lambda)?;
                let unboxed = self.local(Type::Closure);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Closure(ids, func_id)),
                    Stat::Expr(result, Expr::BoxClosure(unboxed)),
                ]))
            }
            ast::AST::If(cond, then, els) => {
                let boxed_cond_local = self.local(Type::Boxed);
                let cond = self.gen_stat(ir_generator, Some(boxed_cond_local), cond)?;

                // TODO: condがboolかのチェック
                let cond_local = self.local(Type::Bool);
                let unbox_cond = Stat::Expr(Some(cond_local), Expr::UnboxBool(boxed_cond_local));

                let then = self.gen_stat(ir_generator, result, then)?;
                let els = self.gen_stat(ir_generator, result, els)?;
                Ok(Stat::Begin(vec![
                    cond,
                    unbox_cond,
                    Stat::If(cond_local, Box::new(then), Box::new(els)),
                ]))
            }
            ast::AST::Call(func, args) => {
                let mut stats = Vec::new();

                let boxed_func_local = self.local(Type::Boxed);
                let boxed_func = self.gen_stat(ir_generator, Some(boxed_func_local), func)?;
                stats.push(boxed_func);

                // TODO: funcがクロージャかのチェック
                let func_local = self.local(Type::Closure);
                let func = Stat::Expr(Some(func_local), Expr::UnboxClosure(boxed_func_local));
                stats.push(func);

                // TODO: 引数の数が合っているかのチェック
                let mut arg_locals = Vec::new();
                arg_locals.push(func_local); // 第一引数にクロージャを渡す
                for arg in args {
                    let arg_local = self.local(Type::Boxed);
                    let arg = self.gen_stat(ir_generator, Some(arg_local), arg)?;
                    stats.push(arg);
                    arg_locals.push(arg_local);
                }
                stats.push(Stat::Expr(result, Expr::Call(func_local, arg_locals)));
                Ok(Stat::Begin(stats))
            }
            ast::AST::Var(name) => {
                let local = self
                    .local_names
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown variable"))?;
                Ok(Stat::Expr(result, Expr::Move(*local)))
            }
            ast::AST::Begin(stats) => {
                let mut ir_stats = Vec::new();
                for stat in stats {
                    let ir_stat = self.gen_stat(ir_generator, result, stat)?;
                    ir_stats.push(ir_stat);
                }
                if ir_stats.is_empty() {
                    // goshと同じようにbeginの中身が空の場合は0を返す
                    ir_stats.push(Stat::Expr(result, Expr::Int(0)));
                }
                Ok(Stat::Begin(ir_stats))
            }
            ast::AST::Dump(expr) => {
                let boxed_local = self.local(Type::Boxed);
                let expr = self.gen_stat(ir_generator, Some(boxed_local), expr)?;
                Ok(Stat::Begin(vec![
                    expr,
                    Stat::Expr(result, Expr::Dump(boxed_local)),
                ]))
            }
        }
    }

    fn quote(&mut self, result: Option<usize>, sexpr: &sexpr::SExpr) -> Result<Stat> {
        match sexpr {
            sexpr::SExpr::Bool(b) => {
                let unboxed = self.local(Type::Bool);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Bool(*b)),
                    Stat::Expr(result, Expr::BoxBool(unboxed)),
                ]))
            }
            sexpr::SExpr::Int(i) => {
                let unboxed = self.local(Type::Int);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Int(*i)),
                    Stat::Expr(result, Expr::BoxInt(unboxed)),
                ]))
            }
            sexpr::SExpr::String(s) => {
                let unboxed = self.local(Type::String);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::String(s.clone())),
                    Stat::Expr(result, Expr::BoxString(unboxed)),
                ]))
            }
            sexpr::SExpr::Symbol(s) => {
                let string = self.local(Type::String);
                let unboxed = self.local(Type::Symbol);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(string), Expr::String(s.clone())),
                    Stat::Expr(Some(unboxed), Expr::StringToSymbol(string)),
                    Stat::Expr(result, Expr::BoxSymbol(unboxed)),
                ]))
            }
            sexpr::SExpr::Nil => {
                let unboxed = self.local(Type::Nil);
                Ok(Stat::Begin(vec![
                    Stat::Expr(Some(unboxed), Expr::Nil),
                    Stat::Expr(result, Expr::BoxNil(unboxed)),
                ]))
            }
            sexpr::SExpr::Cons(cons) => {
                let car_local = self.local(Type::Boxed);
                let car = self.quote(Some(car_local), &cons.car)?;
                let cdr_local = self.local(Type::Boxed);
                let cdr: Stat = self.quote(Some(cdr_local), &cons.cdr)?;

                let unboxed = self.local(Type::Cons);
                let cons = Stat::Expr(Some(unboxed), Expr::Cons(car_local, cdr_local));

                Ok(Stat::Begin(vec![
                    car,
                    cdr,
                    cons,
                    Stat::Expr(result, Expr::BoxCons(unboxed)),
                ]))
            }
        }
    }

    fn local(&mut self, typ: Type) -> usize {
        let local = self.locals.len();
        self.locals.push(typ);
        local
    }

    fn named_local(&mut self, name: String) -> usize {
        let local = self.local(Type::Boxed);
        self.local_names.insert(name, local);
        local
    }
}
