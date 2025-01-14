use std::collections::HashMap;

use crate::{ast, sexpr};
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    String(String),
    Symbol(String),
    Nil,
    Cons(usize, usize),
    Closure(Vec<usize>, usize),
    /*
    set! が未実装なので一旦実装しない
    MutCell(usize),
    MutCellDeref(usize),*/
    Call(usize, Vec<usize>),
    Move(usize),
}

#[derive(Debug, Clone)]
pub enum Stat {
    If(usize, Box<Stat>, Box<Stat>),
    Begin(Vec<Stat>),
    Expr(Option<usize>, Expr),
}

#[derive(Debug, Clone)]
pub struct Func {
    pub envs: usize,
    // 自信のクロージャを含む(+1される)
    pub args: usize,
    // argsを含む
    pub locals: usize,
    pub ret: usize,
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
        Ok(Ir {
            funcs: self.funcs,
            entry,
        })
    }
}

#[derive(Debug, Clone)]
struct LambdaGenerator {
    locals: usize,
    local_names: HashMap<String, usize>,
}

impl LambdaGenerator {
    fn new() -> Self {
        Self {
            locals: 0,
            local_names: HashMap::new(),
        }
    }

    fn gen(
        mut self,
        ir_generator: &mut IrGenerator,
        envs: Vec<String>,
        lambda: &ast::Lambda,
    ) -> Result<usize> {
        self.locals = 1; // for closure
        for arg in &lambda.args {
            self.named_local(arg.clone());
        }
        for env in &envs {
            self.named_local(env.clone());
        }
        let ret = self.local();
        let body = self.gen_stat(ir_generator, Some(ret), &*lambda.body)?;
        let func = Func {
            envs: envs.len(),
            args: lambda.args.len() + 1,
            locals: self.locals,
            ret,
            body,
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
            ast::AST::Bool(b) => Ok(Stat::Expr(result, Expr::Bool(*b))),
            ast::AST::Int(i) => Ok(Stat::Expr(result, Expr::Int(*i))),
            ast::AST::String(s) => Ok(Stat::Expr(result, Expr::String(s.clone()))),
            ast::AST::Nil => Ok(Stat::Expr(result, Expr::Nil)),
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
                Ok(Stat::Expr(result, Expr::Closure(ids, func_id)))
            }
            ast::AST::If(cond, then, els) => {
                let cond_local = self.local();
                let cond = self.gen_stat(ir_generator, Some(cond_local), cond)?;
                let then = self.gen_stat(ir_generator, result, then)?;
                let els = self.gen_stat(ir_generator, result, els)?;
                Ok(Stat::Begin(vec![
                    cond,
                    Stat::If(cond_local, Box::new(then), Box::new(els)),
                ]))
            }
            ast::AST::Call(func, args) => {
                let mut stats = Vec::new();
                let func_local = self.local();
                let func = self.gen_stat(ir_generator, Some(func_local), func)?;
                stats.push(func);
                let mut arg_locals = Vec::new();
                for arg in args {
                    let arg_local = self.local();
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
                    ir_stats.push(Stat::Expr(result, Expr::Int(0)));
                }
                Ok(Stat::Begin(ir_stats))
            }
        }
    }

    fn quote(&mut self, result: Option<usize>, sexpr: &sexpr::SExpr) -> Result<Stat> {
        match sexpr {
            sexpr::SExpr::Bool(b) => Ok(Stat::Expr(result, Expr::Bool(*b))),
            sexpr::SExpr::Int(i) => Ok(Stat::Expr(result, Expr::Int(*i))),
            sexpr::SExpr::String(s) => Ok(Stat::Expr(result, Expr::String(s.clone()))),
            sexpr::SExpr::Symbol(s) => Ok(Stat::Expr(result, Expr::Symbol(s.clone()))),
            sexpr::SExpr::Nil => Ok(Stat::Expr(result, Expr::Nil)),
            sexpr::SExpr::Cons(cons) => {
                let car_local = self.local();
                let car = self.quote(Some(car_local), &cons.car)?;
                let cdr_local = self.local();
                let cdr = self.quote(Some(cdr_local), &cons.cdr)?;
                let cons = Stat::Expr(result, Expr::Cons(car_local, cdr_local));

                Ok(Stat::Begin(vec![car, cdr, cons]))
            }
        }
    }

    fn local(&mut self) -> usize {
        let local = self.locals;
        self.locals += 1;
        local
    }

    fn named_local(&mut self, name: String) -> usize {
        let local = self.local();
        self.local_names.insert(name, local);
        local
    }
}
