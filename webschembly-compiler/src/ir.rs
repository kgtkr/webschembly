use std::collections::HashMap;

use crate::{ast, sexpr};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
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
    /*
    TODO:
    CallClosureにrename
    もしくは以下の3命令に分割してもよい
    * ClosureFuncRef: Closure -> FuncRef
    * ClosureEnvs: Closure -> [Boxed]
    * CallFuncRef
    */
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
    If(usize, Vec<Stat>, Vec<Stat>),
    Expr(Option<usize>, Expr),
}

#[derive(Debug, Clone)]
pub struct Func {
    pub locals: Vec<Type>,
    // localsの先頭何個が引数か
    pub args: usize,
    // localsのうちどれが返り値か
    pub rets: Vec<usize>,
    pub body: Vec<Stat>,
}

impl Func {
    pub fn arg_types(&self) -> Vec<Type> {
        (0..self.args).map(|i| self.locals[i]).collect()
    }

    pub fn ret_types(&self) -> Vec<Type> {
        self.rets.iter().map(|&ret| self.locals[ret]).collect()
    }

    pub fn func_type(&self) -> FuncType {
        FuncType {
            args: self.arg_types(),
            rets: self.ret_types(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub args: Vec<Type>,
    pub rets: Vec<Type>,
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

#[derive(Debug)]
struct IrGenerator {
    funcs: Vec<Func>,
}

impl IrGenerator {
    fn new() -> Self {
        Self { funcs: Vec::new() }
    }

    fn gen(mut self, ast: &ast::AST) -> Result<Ir> {
        let entry = self.gen_func(
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
            body: vec![
                Stat::Expr(Some(0), Expr::Closure(vec![], entry)),
                Stat::Expr(None, Expr::Call(0, vec![0])),
            ],
        });

        Ok(Ir {
            funcs: self.funcs,
            entry: entry_wrapper,
        })
    }

    fn gen_func(&mut self, envs: Vec<String>, lambda: &ast::Lambda) -> Result<usize> {
        let func = LambdaGenerator::new(self).gen(envs, lambda)?;
        let func_id = self.funcs.len();
        self.funcs.push(func);
        Ok(func_id)
    }
}

#[derive(Debug)]
struct LambdaGenerator<'a> {
    locals: Vec<Type>,
    local_names: HashMap<String, usize>,
    ir_generator: &'a mut IrGenerator,
}

impl<'a> LambdaGenerator<'a> {
    fn new(ir_generator: &'a mut IrGenerator) -> Self {
        Self {
            locals: Vec::new(),
            local_names: HashMap::new(),
            ir_generator,
        }
    }

    fn gen(mut self, envs: Vec<String>, lambda: &ast::Lambda) -> Result<Func> {
        let self_closure = self.local(Type::Closure);
        for arg in &lambda.args {
            self.named_local(arg.clone());
        }

        let mut restore_envs = Vec::new();
        // クロージャから環境を復元
        for (i, env) in envs.iter().enumerate() {
            let env_local = self.named_local(env.clone());
            restore_envs.push(Stat::Expr(
                Some(env_local),
                Expr::ClosureEnv(self_closure, i),
            ));
        }

        let ret = self.local(Type::Boxed);
        let body = {
            let mut block_gen = BlockGenerator::new(&mut self);
            block_gen.gen_stat(Some(ret), &*lambda.body)?;
            let mut body = Vec::new();
            body.extend(restore_envs);
            body.extend(block_gen.stats);
            body
        };
        Ok(Func {
            args: lambda.args.len() + 1,
            rets: vec![ret],
            locals: self.locals,
            body,
        })
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

#[derive(Debug)]
struct BlockGenerator<'a, 'b> {
    stats: Vec<Stat>,
    lambda_gen: &'b mut LambdaGenerator<'a>,
}

impl<'a, 'b> BlockGenerator<'a, 'b> {
    fn new(lambda_gen: &'b mut LambdaGenerator<'a>) -> Self {
        Self {
            stats: Vec::new(),
            lambda_gen,
        }
    }

    fn gen_stat(&mut self, result: Option<usize>, ast: &ast::AST) -> Result<()> {
        match ast {
            ast::AST::Bool(b) => {
                let unboxed = self.lambda_gen.local(Type::Bool);
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Bool(*b)));
                self.stats.push(Stat::Expr(result, Expr::BoxBool(unboxed)));
                Ok(())
            }
            ast::AST::Int(i) => {
                let unboxed = self.lambda_gen.local(Type::Int);
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Int(*i)));
                self.stats.push(Stat::Expr(result, Expr::BoxInt(unboxed)));
                Ok(())
            }
            ast::AST::String(s) => {
                let unboxed = self.lambda_gen.local(Type::String);
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::String(s.clone())));
                self.stats
                    .push(Stat::Expr(result, Expr::BoxString(unboxed)));
                Ok(())
            }
            ast::AST::Nil => {
                let unboxed = self.lambda_gen.local(Type::Nil);
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Nil));
                self.stats.push(Stat::Expr(result, Expr::BoxNil(unboxed)));
                Ok(())
            }
            ast::AST::Quote(sexpr) => {
                self.quote(result, sexpr)?;
                Ok(())
            }
            ast::AST::Define(name, expr) => {
                let local = self.lambda_gen.named_local(name.clone());
                self.gen_stat(Some(local), expr)?;
                Ok(())
            }
            ast::AST::Lambda(lambda) => {
                // クロージャで使われているかに関わらず全てのローカル変数を環境に含める
                let local_names = self.lambda_gen.local_names.iter().collect::<Vec<_>>();
                let names = local_names
                    .iter()
                    .map(|&(name, _)| name.clone())
                    .collect::<Vec<_>>();
                let ids = local_names
                    .iter()
                    .map(|&(_, &local)| local)
                    .collect::<Vec<_>>();
                let func_id = self.lambda_gen.ir_generator.gen_func(names, lambda)?;
                let unboxed = self.lambda_gen.local(Type::Closure);
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::Closure(ids, func_id)));
                self.stats
                    .push(Stat::Expr(result, Expr::BoxClosure(unboxed)));
                Ok(())
            }
            ast::AST::If(cond, then, els) => {
                let boxed_cond_local = self.lambda_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_cond_local), cond)?;

                // TODO: condがboolかのチェック
                let cond_local = self.lambda_gen.local(Type::Bool);
                self.stats.push(Stat::Expr(
                    Some(cond_local),
                    Expr::UnboxBool(boxed_cond_local),
                ));

                let then_stats = {
                    let mut then_gen = BlockGenerator::new(self.lambda_gen);
                    then_gen.gen_stat(result, then)?;
                    then_gen.stats
                };

                let else_stats = {
                    let mut els_gen = BlockGenerator::new(self.lambda_gen);
                    els_gen.gen_stat(result, els)?;
                    els_gen.stats
                };

                self.stats
                    .push(Stat::If(cond_local, then_stats, else_stats));

                Ok(())
            }
            ast::AST::Call(func, args) => {
                let boxed_func_local = self.lambda_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_func_local), func)?;

                // TODO: funcがクロージャかのチェック
                let func_local = self.lambda_gen.local(Type::Closure);
                self.stats.push(Stat::Expr(
                    Some(func_local),
                    Expr::UnboxClosure(boxed_func_local),
                ));

                // TODO: 引数の数が合っているかのチェック
                let mut arg_locals = Vec::new();
                arg_locals.push(func_local); // 第一引数にクロージャを渡す
                for arg in args {
                    let arg_local = self.lambda_gen.local(Type::Boxed);
                    self.gen_stat(Some(arg_local), arg)?;
                    arg_locals.push(arg_local);
                }
                self.stats
                    .push(Stat::Expr(result, Expr::Call(func_local, arg_locals)));
                Ok(())
            }
            ast::AST::Var(name) => {
                let local = self
                    .lambda_gen
                    .local_names
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown variable"))?;
                self.stats.push(Stat::Expr(result, Expr::Move(*local)));
                Ok(())
            }
            ast::AST::Begin(stats) => {
                for stat in stats {
                    self.gen_stat(result, stat)?;
                }
                if stats.is_empty() {
                    // goshと同じようにbeginの中身が空の場合は0を返す
                    self.stats.push(Stat::Expr(result, Expr::Int(0)));
                }
                Ok(())
            }
            ast::AST::Dump(expr) => {
                let boxed_local = self.lambda_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_local), expr)?;
                self.stats.push(Stat::Expr(result, Expr::Dump(boxed_local)));
                Ok(())
            }
        }
    }

    fn quote(&mut self, result: Option<usize>, sexpr: &sexpr::SExpr) -> Result<()> {
        match sexpr {
            sexpr::SExpr::Bool(b) => {
                let unboxed = self.lambda_gen.local(Type::Bool);
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Bool(*b)));
                self.stats.push(Stat::Expr(result, Expr::BoxBool(unboxed)));
                Ok(())
            }
            sexpr::SExpr::Int(i) => {
                let unboxed = self.lambda_gen.local(Type::Int);
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Int(*i)));
                self.stats.push(Stat::Expr(result, Expr::BoxInt(unboxed)));
                Ok(())
            }
            sexpr::SExpr::String(s) => {
                let unboxed = self.lambda_gen.local(Type::String);
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::String(s.clone())));
                self.stats
                    .push(Stat::Expr(result, Expr::BoxString(unboxed)));
                Ok(())
            }
            sexpr::SExpr::Symbol(s) => {
                let string = self.lambda_gen.local(Type::String);
                let unboxed = self.lambda_gen.local(Type::Symbol);
                self.stats
                    .push(Stat::Expr(Some(string), Expr::String(s.clone())));
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::StringToSymbol(string)));
                self.stats
                    .push(Stat::Expr(result, Expr::BoxSymbol(unboxed)));
                Ok(())
            }
            sexpr::SExpr::Nil => {
                let unboxed = self.lambda_gen.local(Type::Nil);
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Nil));
                self.stats.push(Stat::Expr(result, Expr::BoxNil(unboxed)));
                Ok(())
            }
            sexpr::SExpr::Cons(cons) => {
                let car_local = self.lambda_gen.local(Type::Boxed);
                self.quote(Some(car_local), &cons.car)?;
                let cdr_local = self.lambda_gen.local(Type::Boxed);
                self.quote(Some(cdr_local), &cons.cdr)?;

                let unboxed = self.lambda_gen.local(Type::Cons);
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::Cons(car_local, cdr_local)));
                self.stats.push(Stat::Expr(result, Expr::BoxCons(unboxed)));
                Ok(())
            }
        }
    }
}
