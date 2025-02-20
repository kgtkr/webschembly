use std::collections::HashMap;

use crate::{ast, sexpr, x::RunX};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Type {
    Boxed,
    Val(ValType),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum ValType {
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
    CallClosure(usize, Vec<usize>),
    Move(usize),
    Box(ValType, usize),
    Unbox(ValType, usize),
    Dump(usize),
    ClosureEnv(usize /* closure */, usize /* env index */),
    GlobalSet(usize, usize),
    GlobalGet(usize),
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
    pub global_count: usize,
    pub entry: usize,
}

impl Ir {
    pub fn from_ast(ast: &ast::Ast<ast::Final>) -> Result<Ir> {
        let ir_gen = IrGenerator::new();

        Ok(ir_gen.gen(ast)?)
    }
}

#[derive(Debug)]
struct IrGenerator {
    funcs: Vec<Func>,
    global_ids: HashMap<ast::GlobalVarId, usize>,
}

impl IrGenerator {
    fn new() -> Self {
        Self {
            funcs: Vec::new(),
            global_ids: HashMap::new(),
        }
    }

    fn gen(mut self, ast: &ast::Ast<ast::Final>) -> Result<Ir> {
        let func = FuncGenerator::new(&mut self).entry_gen(ast)?;
        let func_id = self.funcs.len();
        self.funcs.push(func);

        Ok(Ir {
            funcs: self.funcs,
            entry: func_id,
            global_count: self.global_ids.len(),
        })
    }

    fn gen_func(
        &mut self,
        x: RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> Result<usize> {
        let func = FuncGenerator::new(self).lambda_gen(x, lambda)?;
        let func_id = self.funcs.len();
        self.funcs.push(func);
        Ok(func_id)
    }

    fn global_id(&mut self, id: ast::GlobalVarId) -> usize {
        if let Some(&global_id) = self.global_ids.get(&id) {
            global_id
        } else {
            let global_id = self.global_ids.len();
            self.global_ids.insert(id, global_id);
            global_id
        }
    }
}

#[derive(Debug)]
struct FuncGenerator<'a> {
    locals: Vec<Type>,
    local_ids: HashMap<ast::LocalVarId, usize>,
    ir_generator: &'a mut IrGenerator,
}

impl<'a> FuncGenerator<'a> {
    fn new(ir_generator: &'a mut IrGenerator) -> Self {
        Self {
            locals: Vec::new(),
            local_ids: HashMap::new(),
            ir_generator,
        }
    }

    fn entry_gen(mut self, ast: &ast::Ast<ast::Final>) -> Result<Func> {
        let body = {
            let mut block_gen = BlockGenerator::new(&mut self);
            block_gen.gen_stats(None, &ast.exprs)?;
            block_gen.stats
        };
        Ok(Func {
            args: 0,
            rets: vec![],
            locals: self.locals,
            body,
        })
    }

    fn lambda_gen(
        mut self,
        x: RunX<ast::LambdaX, ast::Final>,
        lambda: &ast::Lambda<ast::Final>,
    ) -> Result<Func> {
        let self_closure = self.local(Type::Val(ValType::Closure));
        for &arg in &x.args {
            self.define_ast_local(arg);
        }

        let mut restore_envs = Vec::new();
        // クロージャから環境を復元
        for (i, env) in x.captures.iter().enumerate() {
            let env_local = self.define_ast_local(env.clone());
            restore_envs.push(Stat::Expr(
                Some(env_local),
                Expr::ClosureEnv(self_closure, i),
            ));
        }

        for id in x.defines {
            self.define_ast_local(id);
        }

        let ret = self.local(Type::Boxed);
        let body = {
            let mut block_gen = BlockGenerator::new(&mut self);
            block_gen.gen_stats(Some(ret), &lambda.body)?;
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

    fn define_ast_local(&mut self, id: ast::LocalVarId) -> usize {
        let local: usize = self.local(Type::Boxed);
        self.local_ids.insert(id, local);
        local
    }
}

#[derive(Debug)]
struct BlockGenerator<'a, 'b> {
    stats: Vec<Stat>,
    func_gen: &'b mut FuncGenerator<'a>,
}

impl<'a, 'b> BlockGenerator<'a, 'b> {
    fn new(func_gen: &'b mut FuncGenerator<'a>) -> Self {
        Self {
            stats: Vec::new(),
            func_gen,
        }
    }

    fn gen_stat(&mut self, result: Option<usize>, ast: &ast::Expr<ast::Final>) -> Result<()> {
        match ast {
            ast::Expr::Literal(_, lit) => match lit {
                ast::Literal::Bool(b) => {
                    let unboxed = self.func_gen.local(Type::Val(ValType::Bool));
                    self.stats.push(Stat::Expr(Some(unboxed), Expr::Bool(*b)));
                    self.stats
                        .push(Stat::Expr(result, Expr::Box(ValType::Bool, unboxed)));
                    Ok(())
                }
                ast::Literal::Int(i) => {
                    let unboxed = self.func_gen.local(Type::Val(ValType::Int));
                    self.stats.push(Stat::Expr(Some(unboxed), Expr::Int(*i)));
                    self.stats
                        .push(Stat::Expr(result, Expr::Box(ValType::Int, unboxed)));
                    Ok(())
                }
                ast::Literal::String(s) => {
                    let unboxed = self.func_gen.local(Type::Val(ValType::String));
                    self.stats
                        .push(Stat::Expr(Some(unboxed), Expr::String(s.clone())));
                    self.stats
                        .push(Stat::Expr(result, Expr::Box(ValType::String, unboxed)));
                    Ok(())
                }
                ast::Literal::Nil => {
                    let unboxed = self.func_gen.local(Type::Val(ValType::Nil));
                    self.stats.push(Stat::Expr(Some(unboxed), Expr::Nil));
                    self.stats
                        .push(Stat::Expr(result, Expr::Box(ValType::Nil, unboxed)));
                    Ok(())
                }
                ast::Literal::Quote(sexpr) => {
                    self.quote(result, sexpr)?;
                    Ok(())
                }
            },
            ast::Expr::Define(x, ast::Define { name: _, expr }) => match x.var_id {
                ast::VarId::Local(id) => {
                    let local = self.func_gen.local_ids.get(&id).unwrap();
                    self.gen_stat(Some(*local), expr)?;
                    Ok(())
                }
                ast::VarId::Global(id) => {
                    let local = self.func_gen.local(Type::Boxed);
                    self.gen_stat(Some(local), expr)?;
                    self.stats.push(Stat::Expr(
                        None,
                        Expr::GlobalSet(self.func_gen.ir_generator.global_id(id), local),
                    ));
                    Ok(())
                }
            },
            ast::Expr::Lambda(x, lambda) => {
                let captures = x
                    .captures
                    .iter()
                    .map(|id| *self.func_gen.local_ids.get(id).unwrap())
                    .collect::<Vec<_>>();
                let func_id: usize = self.func_gen.ir_generator.gen_func(x.clone(), lambda)?;
                let unboxed = self.func_gen.local(Type::Val(ValType::Closure));
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::Closure(captures, func_id)));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::Closure, unboxed)));
                Ok(())
            }
            ast::Expr::If(_, ast::If { cond, then, els }) => {
                let boxed_cond_local = self.func_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_cond_local), cond)?;

                // TODO: condがboolかのチェック
                let cond_local = self.func_gen.local(Type::Val(ValType::Bool));
                self.stats.push(Stat::Expr(
                    Some(cond_local),
                    Expr::Unbox(ValType::Bool, boxed_cond_local),
                ));

                let then_stats = {
                    let mut then_gen = BlockGenerator::new(self.func_gen);
                    then_gen.gen_stat(result, then)?;
                    then_gen.stats
                };

                let else_stats = {
                    let mut els_gen = BlockGenerator::new(self.func_gen);
                    els_gen.gen_stat(result, els)?;
                    els_gen.stats
                };

                self.stats
                    .push(Stat::If(cond_local, then_stats, else_stats));

                Ok(())
            }
            ast::Expr::Call(_, ast::Call { func, args }) => {
                let boxed_func_local = self.func_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_func_local), func)?;

                // TODO: funcがクロージャかのチェック
                let func_local = self.func_gen.local(Type::Val(ValType::Closure));
                self.stats.push(Stat::Expr(
                    Some(func_local),
                    Expr::Unbox(ValType::Closure, boxed_func_local),
                ));

                // TODO: 引数の数が合っているかのチェック
                let mut arg_locals = Vec::new();
                arg_locals.push(func_local); // 第一引数にクロージャを渡す
                for arg in args {
                    let arg_local = self.func_gen.local(Type::Boxed);
                    self.gen_stat(Some(arg_local), arg)?;
                    arg_locals.push(arg_local);
                }
                self.stats.push(Stat::Expr(
                    result,
                    Expr::CallClosure(func_local, arg_locals),
                ));
                Ok(())
            }
            ast::Expr::Var(x, _) => match x.var_id {
                ast::VarId::Local(id) => {
                    self.stats.push(Stat::Expr(
                        result,
                        Expr::Move(*self.func_gen.local_ids.get(&id).unwrap()),
                    ));
                    Ok(())
                }
                ast::VarId::Global(id) => {
                    let global_id = self.func_gen.ir_generator.global_id(id);
                    self.stats
                        .push(Stat::Expr(result, Expr::GlobalGet(global_id)));
                    Ok(())
                }
            },
            ast::Expr::Begin(_, ast::Begin { exprs: stats }) => {
                let mut block_gen = BlockGenerator::new(self.func_gen);
                block_gen.gen_stats(result, stats)?;
                self.stats.extend(block_gen.stats);
                Ok(())
            }
            ast::Expr::Dump(_, expr) => {
                let boxed_local = self.func_gen.local(Type::Boxed);
                self.gen_stat(Some(boxed_local), expr)?;
                self.stats.push(Stat::Expr(result, Expr::Dump(boxed_local)));
                Ok(())
            }
        }
    }

    fn quote(&mut self, result: Option<usize>, sexpr: &sexpr::SExpr) -> Result<()> {
        match sexpr {
            sexpr::SExpr::Bool(b) => {
                let unboxed = self.func_gen.local(Type::Val(ValType::Bool));
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Bool(*b)));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::Bool, unboxed)));
                Ok(())
            }
            sexpr::SExpr::Int(i) => {
                let unboxed = self.func_gen.local(Type::Val(ValType::Int));
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Int(*i)));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::Int, unboxed)));
                Ok(())
            }
            sexpr::SExpr::String(s) => {
                let unboxed = self.func_gen.local(Type::Val(ValType::String));
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::String(s.clone())));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::String, unboxed)));
                Ok(())
            }
            sexpr::SExpr::Symbol(s) => {
                let string = self.func_gen.local(Type::Val(ValType::String));
                let unboxed = self.func_gen.local(Type::Val(ValType::Symbol));
                self.stats
                    .push(Stat::Expr(Some(string), Expr::String(s.clone())));
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::StringToSymbol(string)));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::Symbol, unboxed)));
                Ok(())
            }
            sexpr::SExpr::Nil => {
                let unboxed = self.func_gen.local(Type::Val(ValType::Nil));
                self.stats.push(Stat::Expr(Some(unboxed), Expr::Nil));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::Nil, unboxed)));
                Ok(())
            }
            sexpr::SExpr::Cons(cons) => {
                let car_local = self.func_gen.local(Type::Boxed);
                self.quote(Some(car_local), &cons.car)?;
                let cdr_local = self.func_gen.local(Type::Boxed);
                self.quote(Some(cdr_local), &cons.cdr)?;

                let unboxed = self.func_gen.local(Type::Val(ValType::Cons));
                self.stats
                    .push(Stat::Expr(Some(unboxed), Expr::Cons(car_local, cdr_local)));
                self.stats
                    .push(Stat::Expr(result, Expr::Box(ValType::Cons, unboxed)));
                Ok(())
            }
        }
    }

    fn gen_stats(
        &mut self,
        result: Option<usize>,
        stats: &Vec<ast::Expr<ast::Final>>,
    ) -> Result<()> {
        if let Some((last, rest)) = stats.split_last() {
            for stat in rest {
                self.gen_stat(None, stat)?;
            }
            self.gen_stat(result, last)?;
        } else {
            // goshと同じようにbeginの中身が空の場合は0を返す
            self.stats.push(Stat::Expr(result, Expr::Int(0)));
        }
        Ok(())
    }
}
