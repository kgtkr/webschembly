use rustc_hash::FxHashMap;
use webschembly_compiler_ast_generator::{Final, GlobalVarId, LocalVarId, UsedExtR, VarId};
use webschembly_compiler_locate::Located;

use crate::ir_generator::GlobalManager;
use vec_map::VecMap;
use webschembly_compiler_ast as ast;
use webschembly_compiler_ast::AstPhase;
use webschembly_compiler_ir::*;

#[derive(Debug, Clone)]
pub struct Config {
    pub allow_set_builtin: bool,
}

pub fn generate_module(
    id: JitModuleId,
    global_manager: &mut GlobalManager,
    ast: &ast::Ast<Final>,
    config: Config,
) -> Module {
    let module_gen = ModuleGenerator::new(id, config, global_manager, ast);

    module_gen.generate()
}

#[derive(Debug)]
struct ModuleGenerator<'a> {
    id: JitModuleId,
    global_manager: &'a mut GlobalManager,
    ast: &'a ast::Ast<Final>,
    funcs: VecMap<FuncId, Func>,
    config: Config,
    func_to_entrypoint_table: FxHashMap<FuncId, GlobalId>,
    globals: FxHashMap<GlobalId, Global>,
    // メタ情報
    local_metas: FxHashMap<(FuncId, LocalId), VarMeta>,
    global_metas: FxHashMap<GlobalId, VarMeta>,
}

impl<'a> ModuleGenerator<'a> {
    fn new(
        id: JitModuleId,
        config: Config,
        ir_generator: &'a mut GlobalManager,
        ast: &'a ast::Ast<Final>,
    ) -> Self {
        Self {
            id,
            ast,
            global_manager: ir_generator,
            funcs: VecMap::new(),
            config,
            local_metas: FxHashMap::default(),
            global_metas: FxHashMap::default(),
            func_to_entrypoint_table: FxHashMap::default(),
            globals: FxHashMap::default(),
        }
    }

    fn generate(mut self) -> Module {
        let ast_globals = self
            .ast
            .x
            .global_vars
            .clone()
            .into_iter()
            .map(|id| {
                let global = self.global(id);
                (global.id, global)
            })
            .collect::<FxHashMap<_, _>>();
        self.globals.extend(ast_globals);

        let entry_func_id = self.funcs.allocate_key();
        let mut entry_func = FuncGenerator::new(&mut self, entry_func_id).entry_gen();

        // エントリーポイントにモジュール初期化ロジックを追加
        let prev_bb_entry = entry_func.bb_entry;
        let mut entry_exprs = Vec::new();

        for (func_id, entrypoint_table_global_id) in self.func_to_entrypoint_table {
            let func_ref_local = entry_func.locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });
            let mut_func_ref_local = entry_func.locals.push_with(|id| Local {
                id,
                typ: LocalType::MutFuncRef,
            });
            let entrypoint_table_local = entry_func.locals.push_with(|id| Local {
                id,
                typ: LocalType::EntrypointTable,
            });
            entry_exprs.push(Instr {
                local: Some(func_ref_local),
                kind: InstrKind::FuncRef(func_id),
            });
            entry_exprs.push(Instr {
                local: Some(mut_func_ref_local),
                kind: InstrKind::CreateMutFuncRef(func_ref_local),
            });
            entry_exprs.push(Instr {
                local: Some(entrypoint_table_local),
                kind: InstrKind::EntrypointTable(vec![mut_func_ref_local]),
            });
            entry_exprs.push(Instr {
                local: None,
                kind: InstrKind::GlobalSet(entrypoint_table_global_id, entrypoint_table_local),
            });
        }
        entry_exprs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(TerminatorInstr::Jump(prev_bb_entry)),
        });
        let new_bb_entry = entry_func.bbs.push_with(|bb_id| BasicBlock {
            id: bb_id,
            instrs: entry_exprs,
        });
        entry_func.bb_entry = new_bb_entry;

        self.funcs.insert_node(entry_func);

        let meta = Meta {
            local_metas: self.local_metas,
            global_metas: self.global_metas,
        };

        Module {
            globals: self.globals,
            funcs: self.funcs,
            entry: entry_func_id,
            meta,
        }
    }

    fn global(&mut self, id: GlobalVarId) -> Global {
        let global = self.global_manager.global(id);
        let ast_meta = self.ast.x.global_metas.get(&id);
        if let Some(ast_meta) = ast_meta {
            self.global_metas
                .entry(global.id)
                .or_insert_with(|| VarMeta {
                    name: ast_meta.name.clone(),
                });
        }
        global
    }

    fn gen_func(
        &mut self,
        x: &<Final as AstPhase>::XLambda,
        lambda: &ast::Lambda<Final>,
    ) -> FuncId {
        let id = self.funcs.allocate_key();
        let func = FuncGenerator::new(self, id).lambda_gen(x, lambda);
        self.funcs.insert_node(func);

        id
    }
}

#[derive(Debug)]
struct FuncGenerator<'a, 'b> {
    id: FuncId,
    locals: VecMap<LocalId, Local>,
    local_ids: FxHashMap<LocalVarId, LocalId>,
    bbs: VecMap<BasicBlockId, BasicBlock>,
    module_generator: &'a mut ModuleGenerator<'b>,
    exprs: Vec<Instr>,
    current_bb_id: Option<BasicBlockId>,
    uninitialized_vars: FxHashMap<LocalVarId, Vec<UninitializedVarCaptureClosure>>,
}

// クロージャにキャプチャされているが、まだ初期化されていない変数
#[derive(Debug, Clone)]
struct UninitializedVarCaptureClosure {
    closure: LocalId,
    env_index: usize,
    env_types: Vec<LocalType>,
}

impl<'a, 'b> FuncGenerator<'a, 'b> {
    fn new(module_generator: &'a mut ModuleGenerator<'b>, id: FuncId) -> Self {
        Self {
            id,
            locals: VecMap::new(),
            local_ids: FxHashMap::default(),
            bbs: VecMap::new(),
            module_generator,
            exprs: Vec::new(),
            current_bb_id: None,
            uninitialized_vars: FxHashMap::default(),
        }
    }

    fn entry_gen(mut self) -> Func {
        let obj_local = self.local(Type::Obj);

        self.define_all_ast_local_and_create_ref(&self.module_generator.ast.x.defines);

        let bb_entry = self.bbs.allocate_key();
        self.current_bb_id = Some(bb_entry);
        self.gen_exprs(Some(obj_local), &self.module_generator.ast.exprs);
        self.close_bb(TerminatorInstr::Exit(ExitInstr::Return(obj_local)));

        Func {
            id: self.id,
            args: vec![],
            ret_type: LocalType::Type(Type::Obj),
            locals: self.locals,
            bb_entry,
            bbs: self.bbs,
        }
    }

    fn lambda_gen(mut self, x: &<Final as AstPhase>::XLambda, lambda: &ast::Lambda<Final>) -> Func {
        let bb_entry = self.bbs.allocate_key();
        self.current_bb_id = Some(bb_entry);

        let self_closure = self.local(Type::Val(ValType::Closure));
        let args = self.local(LocalType::VariadicArgs);
        let args_len_local = self.local(Type::Val(ValType::Int));
        let expected_args_len_local = self.local(Type::Val(ValType::Int));
        let args_len_check_success_local = self.local(Type::Val(ValType::Bool));

        self.exprs.push(Instr {
            local: Some(args_len_local),
            kind: InstrKind::VariadicArgsLength(args),
        });
        self.exprs.push(Instr {
            local: Some(expected_args_len_local),
            kind: InstrKind::Int(lambda.args.len() as i64),
        });
        self.exprs.push(Instr {
            local: Some(args_len_check_success_local),
            kind: InstrKind::EqInt(args_len_local, expected_args_len_local),
        });

        let error_bb_id = self.bbs.allocate_key();
        let merge_bb_id = self.bbs.allocate_key();
        self.close_bb(TerminatorInstr::If(
            args_len_check_success_local,
            merge_bb_id,
            error_bb_id,
        ));
        self.current_bb_id = Some(error_bb_id);
        let msg = self.local(Type::Val(ValType::String));
        self.exprs.push(Instr {
            local: Some(msg),
            kind: InstrKind::String("args count mismatch\n".to_string()),
        });
        self.close_bb(TerminatorInstr::Exit(ExitInstr::Error(msg)));
        self.current_bb_id = Some(merge_bb_id);

        for (arg_idx, arg) in x.args.iter().enumerate() {
            if self.module_generator.ast.x.box_vars.contains(arg) {
                let local = self.local(LocalType::Type(Type::Obj));
                self.exprs.push(Instr {
                    local: Some(local),
                    kind: InstrKind::VariadicArgsRef(args, arg_idx),
                });

                let ref_ = self.define_ast_local(*arg);
                self.exprs.push(Instr {
                    local: Some(ref_),
                    kind: InstrKind::CreateRef(Type::Obj),
                });
                self.exprs.push(Instr {
                    local: None,
                    kind: InstrKind::SetRef(Type::Obj, ref_, local),
                });
            } else {
                let arg_id = self.define_ast_local(*arg);
                self.exprs.push(Instr {
                    local: Some(arg_id),
                    kind: InstrKind::VariadicArgsRef(args, arg_idx),
                });
            }
        }

        // 環境を復元するためのローカル変数を定義
        for var_id in x.captures.iter() {
            self.define_ast_local(*var_id);
        }
        // 環境の型を収集
        let env_types = x
            .captures
            .iter()
            .map(|id| self.locals[*self.local_ids.get(id).unwrap()].typ)
            .collect::<Vec<_>>();
        // 環境を復元する処理を追加
        for (i, var_id) in x.captures.iter().enumerate() {
            let env_local = *self.local_ids.get(var_id).unwrap();
            self.exprs.push(Instr {
                local: Some(env_local),
                // TODO: 無駄なclone。Irの設計を見直す
                kind: InstrKind::ClosureEnv(env_types.clone(), self_closure, i),
            });
        }

        self.define_all_ast_local_and_create_ref(&x.defines);

        let ret = self.local(Type::Obj);
        self.gen_exprs(Some(ret), &lambda.body);
        self.close_bb(TerminatorInstr::Exit(ExitInstr::Return(ret)));
        Func {
            id: self.id,
            args: vec![self_closure, args],
            ret_type: LocalType::Type(Type::Obj),
            locals: self.locals,
            bb_entry,
            bbs: self.bbs,
        }
    }

    fn local<T: Into<LocalType>>(&mut self, typ: T) -> LocalId {
        self.locals.push_with(|id| Local {
            id,
            typ: typ.into(),
        })
    }

    fn define_all_ast_local_and_create_ref(&mut self, locals: &[LocalVarId]) {
        for id in locals {
            let local = self.define_ast_local(*id);
            if self.module_generator.ast.x.box_vars.contains(id) {
                self.exprs.push(Instr {
                    local: Some(local),
                    kind: InstrKind::CreateRef(Type::Obj),
                });
            }
        }
    }

    fn define_ast_local(&mut self, id: LocalVarId) -> LocalId {
        let ast_meta = self.module_generator.ast.x.local_metas.get(&id);
        let local = self.local(if self.module_generator.ast.x.box_vars.contains(&id) {
            LocalType::Ref(Type::Obj)
        } else {
            LocalType::Type(Type::Obj)
        });
        let prev = self.local_ids.insert(id, local);
        debug_assert!(prev.is_none());
        if let Some(ast_meta) = ast_meta {
            self.module_generator.local_metas.insert(
                (self.id, local),
                VarMeta {
                    name: ast_meta.name.clone(),
                },
            );
        }
        local
    }

    fn new_version_ast_local(&mut self, id: LocalVarId) -> LocalId {
        let ast_meta = self.module_generator.ast.x.local_metas.get(&id);
        debug_assert!(!self.module_generator.ast.x.box_vars.contains(&id));
        let local = self.local(LocalType::Type(Type::Obj));
        self.local_ids.insert(id, local);
        if let Some(ast_meta) = ast_meta {
            self.module_generator.local_metas.insert(
                (self.id, local),
                VarMeta {
                    name: ast_meta.name.clone(),
                },
            );
        }
        local
    }

    fn gen_expr(&mut self, result: Option<LocalId>, ast: &ast::LExpr<Final>) {
        match &ast.value {
            ast::Expr::Const(_, lit) => match lit {
                ast::Const::Bool(b) => {
                    let val_type_local = self.local(Type::Val(ValType::Bool));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::Bool(*b),
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Bool, val_type_local),
                    });
                }
                ast::Const::Int(i) => {
                    let val_type_local = self.local(Type::Val(ValType::Int));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::Int(*i),
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Int, val_type_local),
                    });
                }
                ast::Const::Float(f) => {
                    let val_type_local = self.local(Type::Val(ValType::Float));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::Float(*f),
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Float, val_type_local),
                    });
                }
                ast::Const::NaN => {
                    let val_type_local = self.local(Type::Val(ValType::Float));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::NaN,
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Float, val_type_local),
                    });
                }
                ast::Const::String(s) => {
                    let val_type_local = self.local(Type::Val(ValType::String));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::String(s.clone()),
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::String, val_type_local),
                    });
                }
                ast::Const::Nil => {
                    let val_type_local = self.local(Type::Val(ValType::Nil));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::Nil,
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Nil, val_type_local),
                    });
                }
                ast::Const::Char(c) => {
                    let val_type_local = self.local(Type::Val(ValType::Char));
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::Char(*c),
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Char, val_type_local),
                    });
                }
                ast::Const::Symbol(s) => {
                    let string = self.local(Type::Val(ValType::String));
                    let val_type_local = self.local(Type::Val(ValType::Symbol));
                    self.exprs.push(Instr {
                        local: Some(string),
                        kind: InstrKind::String(s.clone()),
                    });
                    self.exprs.push(Instr {
                        local: Some(val_type_local),
                        kind: InstrKind::StringToSymbol(string),
                    });
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::ToObj(ValType::Symbol, val_type_local),
                    });
                }
            },
            ast::Expr::Define(x, _) => *x,
            ast::Expr::Lambda(x, lambda) => {
                let func_id = self.module_generator.gen_func(x, lambda);
                let func_local = self.local(LocalType::FuncRef);
                let val_type_local = self.local(Type::Val(ValType::Closure));
                self.exprs.push(Instr {
                    local: Some(func_local),
                    kind: InstrKind::FuncRef(func_id),
                });

                let entrypoint_table_global = self
                    .module_generator
                    .global_manager
                    .gen_global(LocalType::EntrypointTable);
                self.module_generator
                    .globals
                    .insert(entrypoint_table_global.id, entrypoint_table_global);
                self.module_generator
                    .func_to_entrypoint_table
                    .insert(func_id, entrypoint_table_global.id);

                let entrypoint_table_local = self.local(LocalType::EntrypointTable);
                self.exprs.push(Instr {
                    local: Some(entrypoint_table_local),
                    kind: InstrKind::GlobalGet(entrypoint_table_global.id),
                });

                let env_types = x
                    .captures
                    .iter()
                    .map(|id| self.locals[*self.local_ids.get(id).unwrap()].typ)
                    .collect::<Vec<_>>();
                let mut captures = Vec::new();
                for (i, capture) in x.captures.iter().enumerate() {
                    if let Some(uninitialized_var_captures) =
                        self.uninitialized_vars.get_mut(capture)
                    {
                        uninitialized_var_captures.push(UninitializedVarCaptureClosure {
                            closure: val_type_local,
                            env_index: i,
                            env_types: env_types.clone(),
                        });
                        captures.push(None);
                    } else {
                        captures.push(Some(*self.local_ids.get(capture).unwrap()));
                    }
                }

                self.exprs.push(Instr {
                    local: Some(val_type_local),
                    kind: InstrKind::Closure {
                        envs: captures,
                        env_types,
                        func_id: JitFuncId::from(func_id),
                        module_id: self.module_generator.id,
                        entrypoint_table: entrypoint_table_local,
                    },
                });
                self.exprs.push(Instr {
                    local: result,
                    kind: InstrKind::ToObj(ValType::Closure, val_type_local),
                });
            }
            ast::Expr::If(_, ast::If { cond, then, els }) => {
                let obj_cond_local = self.local(Type::Obj);
                self.gen_exprs(Some(obj_cond_local), cond);

                let false_local = self.local(Type::Val(ValType::Bool));
                self.exprs.push(Instr {
                    local: Some(false_local),
                    kind: InstrKind::Bool(false),
                });
                let false_obj_local = self.local(Type::Obj);
                self.exprs.push(Instr {
                    local: Some(false_obj_local),
                    kind: InstrKind::ToObj(ValType::Bool, false_local),
                });

                let cond_not_local = self.local(Type::Val(ValType::Bool));
                self.exprs.push(Instr {
                    local: Some(cond_not_local),
                    kind: InstrKind::EqObj(obj_cond_local, false_obj_local),
                });

                let then_bb_id = self.bbs.allocate_key();
                let else_bb_id = self.bbs.allocate_key();
                let merge_bb_id = self.bbs.allocate_key();
                self.close_bb(TerminatorInstr::If(cond_not_local, else_bb_id, then_bb_id));

                let before_locals = self.local_ids.clone();

                self.current_bb_id = Some(then_bb_id);
                let then_result = self.local(Type::Obj);
                self.gen_exprs(Some(then_result), then);
                let then_locals = self.local_ids.clone();
                let then_ended_bb_id = self.current_bb_id;
                self.close_bb(TerminatorInstr::Jump(merge_bb_id));

                self.current_bb_id = Some(else_bb_id);
                let els_result = self.local(Type::Obj);
                self.gen_exprs(Some(els_result), els);
                let els_locals = self.local_ids.clone();
                let els_ended_bb_id = self.current_bb_id;
                self.close_bb(TerminatorInstr::Jump(merge_bb_id));

                self.current_bb_id = Some(merge_bb_id);
                self.exprs.push(Instr {
                    local: result,
                    kind: InstrKind::Phi {
                        incomings: {
                            let mut incomings = Vec::new();
                            if let Some(bb) = then_ended_bb_id {
                                incomings.push(PhiIncomingValue {
                                    bb,
                                    local: then_result,
                                });
                            }
                            if let Some(bb) = els_ended_bb_id {
                                incomings.push(PhiIncomingValue {
                                    bb,
                                    local: els_result,
                                });
                            }
                            incomings
                        },
                        non_exhaustive: false,
                    },
                });

                // thenとelseでset!された変数をphiノードで結合
                for (var_id, before_local) in before_locals {
                    let then_local = *then_locals.get(&var_id).unwrap();
                    let els_local = *els_locals.get(&var_id).unwrap();

                    if then_local != els_local || then_local != before_local {
                        debug_assert_eq!(self.locals[then_local].typ, self.locals[els_local].typ,);
                        debug_assert_eq!(
                            self.locals[then_local].typ,
                            self.locals[before_local].typ,
                        );
                        let phi_local = self.new_version_ast_local(var_id);
                        self.exprs.push(Instr {
                            local: Some(phi_local),
                            kind: InstrKind::Phi {
                                incomings: {
                                    let mut incomings = Vec::new();
                                    if let Some(bb) = then_ended_bb_id {
                                        incomings.push(PhiIncomingValue {
                                            bb,
                                            local: then_local,
                                        });
                                    }
                                    if let Some(bb) = els_ended_bb_id {
                                        incomings.push(PhiIncomingValue {
                                            bb,
                                            local: els_local,
                                        });
                                    }
                                    incomings
                                },
                                non_exhaustive: false,
                            },
                        });
                    }
                }
            }
            ast::Expr::Call(x, ast::Call { func, args }) => {
                if let [
                    Located {
                        value: ast::Expr::Var(x, name),
                        ..
                    },
                ] = func.as_slice()
                    && let VarId::Global(_) = x.var_id
                    && let Some(builtin) = ast::Builtin::from_name(name)
                {
                    let rules = BuiltinConversionRule::from_builtin(builtin)
                        .into_iter()
                        .filter(|rule| rule.arg_count() == args.len())
                        .collect::<Vec<_>>();

                    if rules.is_empty() {
                        let msg = self.local(Type::Val(ValType::String));
                        self.exprs.push(Instr {
                            local: Some(msg),
                            kind: InstrKind::String("builtin args count mismatch\n".to_string()),
                        });
                        self.close_bb(TerminatorInstr::Exit(ExitInstr::Error(msg)));
                    } else {
                        let merge_bb_id = self.bbs.allocate_key();
                        let mut phi_incoming_values = Vec::new();
                        for rule in rules {
                            let mut obj_arg_locals = Vec::new();
                            let mut type_check_success_locals = Vec::new();
                            for (typ, arg) in rule.arg_types().iter().zip(args) {
                                let obj_arg_local = self.local(Type::Obj);
                                self.gen_exprs(Some(obj_arg_local), arg);
                                obj_arg_locals.push(obj_arg_local);
                                if let Type::Val(val_type) = typ {
                                    let type_check_success_local =
                                        self.local(Type::Val(ValType::Bool));
                                    self.exprs.push(Instr {
                                        local: Some(type_check_success_local),
                                        kind: InstrKind::Is(*val_type, obj_arg_local),
                                    });
                                    type_check_success_locals.push(type_check_success_local);
                                }
                            }

                            let mut all_type_check_success_local =
                                self.local(Type::Val(ValType::Bool));
                            self.exprs.push(Instr {
                                local: Some(all_type_check_success_local),
                                kind: InstrKind::Bool(true),
                            });
                            for type_check_success_local in type_check_success_locals {
                                let new_all_type_check_success_local =
                                    self.local(Type::Val(ValType::Bool));
                                self.exprs.push(Instr {
                                    local: Some(new_all_type_check_success_local),
                                    kind: InstrKind::And(
                                        all_type_check_success_local,
                                        type_check_success_local,
                                    ),
                                });
                                all_type_check_success_local = new_all_type_check_success_local;
                            }

                            let then_bb_id = self.bbs.allocate_key();
                            let else_bb_id = self.bbs.allocate_key();

                            self.close_bb(TerminatorInstr::If(
                                all_type_check_success_local,
                                then_bb_id,
                                else_bb_id,
                            ));

                            self.current_bb_id = Some(then_bb_id);

                            let mut arg_locals = Vec::new();
                            for (typ, obj_arg_local) in rule.arg_types().iter().zip(obj_arg_locals)
                            {
                                let arg_local = match typ {
                                    Type::Obj => obj_arg_local,
                                    Type::Val(val_type) => {
                                        let val_type_local = self.local(Type::Val(*val_type));
                                        self.exprs.push(Instr {
                                            local: Some(val_type_local),
                                            kind: InstrKind::FromObj(*val_type, obj_arg_local),
                                        });
                                        val_type_local
                                    }
                                };
                                arg_locals.push(arg_local);
                            }

                            let ret_local = match rule.ret_type() {
                                Type::Obj => self.local(Type::Obj),
                                Type::Val(val_type) => self.local(Type::Val(val_type)),
                            };
                            let builtin_ctx = BuiltinIrGenCtx {
                                exprs: &mut self.exprs,
                                locals: &mut self.locals,
                                dest: ret_local,
                            };
                            match rule {
                                BuiltinConversionRule::Unary { ir_gen, .. } => {
                                    ir_gen(builtin_ctx, arg_locals[0])
                                }
                                BuiltinConversionRule::Binary { ir_gen, .. } => {
                                    ir_gen(builtin_ctx, arg_locals[0], arg_locals[1])
                                }
                                BuiltinConversionRule::Ternary { ir_gen, .. } => {
                                    ir_gen(builtin_ctx, arg_locals[0], arg_locals[1], arg_locals[2])
                                }
                            };

                            let ret_obj_local = self.local(Type::Obj);
                            self.exprs.push(Instr {
                                local: Some(ret_obj_local),
                                kind: match rule.ret_type() {
                                    Type::Obj => InstrKind::Move(ret_local),
                                    Type::Val(val_type) => InstrKind::ToObj(val_type, ret_local),
                                },
                            });
                            phi_incoming_values.push(PhiIncomingValue {
                                bb: then_bb_id,
                                local: ret_obj_local,
                            });

                            self.close_bb(TerminatorInstr::Jump(merge_bb_id));
                            self.current_bb_id = Some(else_bb_id);
                        }

                        let msg = self.local(Type::Val(ValType::String));
                        self.exprs.push(Instr {
                            local: Some(msg),
                            kind: InstrKind::String(format!(
                                "{}: arg type mismatch\n",
                                builtin.name()
                            )),
                        });
                        self.close_bb(TerminatorInstr::Exit(ExitInstr::Error(msg)));
                        self.current_bb_id = Some(merge_bb_id);
                        self.exprs.push(Instr {
                            local: result,
                            kind: InstrKind::Phi {
                                incomings: phi_incoming_values,
                                non_exhaustive: false,
                            },
                        });
                    }
                } else {
                    let obj_func_local = self.local(Type::Obj);
                    self.gen_exprs(Some(obj_func_local), func);

                    // TODO: funcがクロージャかのチェック
                    let closure_local = self.local(ValType::Closure);
                    self.exprs.push(Instr {
                        local: Some(closure_local),
                        kind: InstrKind::FromObj(ValType::Closure, obj_func_local),
                    });

                    let args_local = self.local(LocalType::VariadicArgs);
                    let mut args_locals = Vec::new();
                    for arg in args {
                        let arg_local = self.local(Type::Obj);
                        self.gen_exprs(Some(arg_local), arg);
                        args_locals.push(arg_local);
                    }
                    self.exprs.push(Instr {
                        local: Some(args_local),
                        kind: InstrKind::VariadicArgs(args_locals),
                    });

                    let is_tail = x.is_tail;
                    let call_closure = InstrCallClosure {
                        closure: closure_local,
                        args: vec![args_local],
                        arg_types: vec![LocalType::VariadicArgs],
                        func_index: 0,
                    };
                    if is_tail {
                        self.close_bb(TerminatorInstr::Exit(ExitInstr::TailCallClosure(
                            call_closure,
                        )));
                    } else {
                        self.exprs.push(Instr {
                            local: result,
                            kind: InstrKind::CallClosure(call_closure),
                        });
                    }
                }
            }
            ast::Expr::Var(x, _) => match &x.var_id {
                VarId::Local(id) => {
                    if self.module_generator.ast.x.box_vars.contains(id) {
                        self.exprs.push(Instr {
                            local: result,
                            kind: InstrKind::DerefRef(Type::Obj, *self.local_ids.get(id).unwrap()),
                        });
                    } else {
                        self.exprs.push(Instr {
                            local: result,
                            kind: InstrKind::Move(*self.local_ids.get(id).unwrap()),
                        });
                    }
                }
                VarId::Global(id) => {
                    let global = self.module_generator.global(*id);
                    self.exprs.push(Instr {
                        local: result,
                        kind: InstrKind::GlobalGet(global.id),
                    });
                }
            },
            ast::Expr::Begin(_, ast::Begin { exprs }) => {
                self.gen_exprs(result, exprs);
            }
            ast::Expr::Set(x, ast::Set { name, expr, .. }) => {
                match &x.var_id {
                    VarId::Local(id) => {
                        if self.module_generator.ast.x.box_vars.contains(id) {
                            let obj_local = self.local(Type::Obj);
                            self.gen_exprs(Some(obj_local), expr);
                            let local = self.local_ids.get(id).unwrap();
                            self.exprs.push(Instr {
                                local: None,
                                kind: InstrKind::SetRef(Type::Obj, *local, obj_local),
                            });
                            self.exprs.push(Instr {
                                local: result,
                                kind: InstrKind::Move(obj_local),
                            });
                        } else {
                            // SSA形式のため、新しいローカルを定義して代入する
                            let local = self.new_version_ast_local(*id);
                            self.gen_exprs(Some(local), expr);
                            self.exprs.push(Instr {
                                local: result,
                                kind: InstrKind::Move(local),
                            });
                        }
                    }
                    VarId::Global(id) => {
                        if let Some(_) = ast::Builtin::from_name(&name.value)
                            && !self.module_generator.config.allow_set_builtin
                        {
                            let msg = self.local(Type::Val(ValType::String));
                            self.exprs.push(Instr {
                                local: Some(msg),
                                kind: InstrKind::String(
                                    "set! builtin is not allowed\n".to_string(),
                                ),
                            });
                            self.close_bb(TerminatorInstr::Exit(ExitInstr::Error(msg)));
                        } else {
                            let local = self.local(Type::Obj);
                            self.gen_exprs(Some(local), expr);
                            let global = self.module_generator.global(*id);
                            self.exprs.push(Instr {
                                local: None,
                                kind: InstrKind::GlobalSet(global.id, local),
                            });
                            self.exprs.push(Instr {
                                local: result,
                                kind: InstrKind::Move(local),
                            });
                        }
                    }
                }
            }
            ast::Expr::Let(x, _) => *x,
            ast::Expr::LetStar(x, _) => *x,
            ast::Expr::LetRec(x, _) => *x,
            ast::Expr::Vector(_, vec) => {
                let mut vec_locals = Vec::new();
                for sexpr in vec {
                    let obj_local = self.local(Type::Obj);
                    self.gen_exprs(Some(obj_local), sexpr);
                    vec_locals.push(obj_local);
                }
                let val_type_local = self.local(Type::Val(ValType::Vector));
                self.exprs.push(Instr {
                    local: Some(val_type_local),
                    kind: InstrKind::Vector(vec_locals),
                });
                self.exprs.push(Instr {
                    local: result,
                    kind: InstrKind::ToObj(ValType::Vector, val_type_local),
                });
            }
            ast::Expr::UVector(_, uvec) => {
                let kind = match uvec.kind {
                    ast::UVectorKind::S64 => UVectorKind::S64,
                    ast::UVectorKind::F64 => UVectorKind::F64,
                };

                let mut element_obj_locals = Vec::new();
                for sexpr in &uvec.elements {
                    let obj_local = self.local(Type::Obj);
                    self.gen_exprs(Some(obj_local), sexpr);
                    element_obj_locals.push(obj_local);
                }
                let mut type_check_all_success_local = self.local(Type::Val(ValType::Bool));
                self.exprs.push(Instr {
                    local: Some(type_check_all_success_local),
                    kind: InstrKind::Bool(true),
                });
                for obj_local in &element_obj_locals {
                    let type_check_success_local = self.local(Type::Val(ValType::Bool));
                    self.exprs.push(Instr {
                        local: Some(type_check_success_local),
                        kind: InstrKind::Is(kind.element_type(), *obj_local),
                    });
                    let new_type_check_all_success_local = self.local(Type::Val(ValType::Bool));
                    self.exprs.push(Instr {
                        local: Some(new_type_check_all_success_local),
                        kind: InstrKind::And(
                            type_check_all_success_local,
                            type_check_success_local,
                        ),
                    });
                    type_check_all_success_local = new_type_check_all_success_local;
                }
                let then_bb_id = self.bbs.allocate_key();
                let else_bb_id = self.bbs.allocate_key();

                self.close_bb(TerminatorInstr::If(
                    type_check_all_success_local,
                    then_bb_id,
                    else_bb_id,
                ));

                self.current_bb_id = Some(else_bb_id);
                let msg = self.local(Type::Val(ValType::String));
                self.exprs.push(Instr {
                    local: Some(msg),
                    kind: InstrKind::String(format!(
                        "uvector element type mismatch: expected {:?}\n",
                        kind.element_type()
                    )),
                });
                self.close_bb(TerminatorInstr::Exit(ExitInstr::Error(msg)));
                self.current_bb_id = Some(then_bb_id);
                let mut element_locals = Vec::new();
                for obj_local in element_obj_locals {
                    let elem_local = self.local(kind.element_type());
                    self.exprs.push(Instr {
                        local: Some(elem_local),
                        kind: InstrKind::FromObj(kind.element_type(), obj_local),
                    });
                    element_locals.push(elem_local);
                }
                let uvector_local = self.local(Type::Val(ValType::UVector(kind)));
                self.exprs.push(Instr {
                    local: Some(uvector_local),
                    kind: InstrKind::UVector(kind, element_locals),
                });
                self.exprs.push(Instr {
                    local: result,
                    kind: InstrKind::ToObj(ValType::UVector(kind), uvector_local),
                });
            }
            ast::Expr::Cons(_, cons) => {
                let car_local = self.local(Type::Obj);
                self.gen_exprs(Some(car_local), &cons.car);
                let cdr_local = self.local(Type::Obj);
                self.gen_exprs(Some(cdr_local), &cons.cdr);

                let val_type_local = self.local(Type::Val(ValType::Cons));
                self.exprs.push(Instr {
                    local: Some(val_type_local),
                    kind: InstrKind::Cons(car_local, cdr_local),
                });
                self.exprs.push(Instr {
                    local: result,
                    kind: InstrKind::ToObj(ValType::Cons, val_type_local),
                });
            }
            ast::Expr::Quote(x, _) => *x,
            ast::Expr::Ext(x) => match x {
                UsedExtR::SetGroup(set_group) => {
                    for var_id in &set_group.var_ids {
                        let prev = self.uninitialized_vars.insert(*var_id, Vec::new());
                        debug_assert!(prev.is_none());
                    }
                    self.gen_exprs(result, &set_group.exprs);
                    for var_id in &set_group.var_ids {
                        let captures = self.uninitialized_vars.remove(var_id).unwrap();
                        for capture in captures {
                            self.exprs.push(Instr {
                                local: None,
                                kind: InstrKind::ClosureSetEnv(
                                    capture.env_types.clone(),
                                    capture.closure,
                                    capture.env_index,
                                    *self.local_ids.get(var_id).unwrap(),
                                ),
                            });
                        }
                    }
                }
            },
        }
    }

    fn gen_exprs(&mut self, result: Option<LocalId>, exprs: &[ast::LExpr<Final>]) {
        if let Some((last, rest)) = exprs.split_last() {
            for expr in rest {
                self.gen_expr(None, expr);
            }
            self.gen_expr(result, last);
        } else {
            let val_type_local = self.local(Type::Val(ValType::Nil));
            self.exprs.push(Instr {
                local: Some(val_type_local),
                kind: InstrKind::Nil,
            });
            self.exprs.push(Instr {
                local: result,
                kind: InstrKind::ToObj(ValType::Nil, val_type_local),
            });
        }
    }

    fn close_bb(&mut self, terminator: TerminatorInstr) {
        self.exprs.push(Instr {
            local: None,
            kind: InstrKind::Terminator(terminator),
        });
        let bb_exprs = std::mem::take(&mut self.exprs);
        if let Some(id) = self.current_bb_id {
            self.bbs.insert(
                self.current_bb_id.unwrap(),
                BasicBlock {
                    id,
                    instrs: bb_exprs,
                },
            );
            self.current_bb_id = None;
        } else {
            // self.current_bb_idがNoneのとき到達不能ブロックである
        }
    }
}

#[derive(Debug)]
pub struct BuiltinIrGenCtx<'a> {
    exprs: &'a mut Vec<Instr>,
    locals: &'a mut VecMap<LocalId, Local>,
    dest: LocalId,
}

#[derive(Debug, Clone, Copy)]
pub enum BuiltinConversionRule {
    Unary {
        args: [Type; 1],
        ret: Type,
        ir_gen: fn(BuiltinIrGenCtx, LocalId),
    },
    Binary {
        args: [Type; 2],
        ret: Type,
        ir_gen: fn(BuiltinIrGenCtx, LocalId, LocalId),
    },
    Ternary {
        args: [Type; 3],
        ret: Type,
        ir_gen: fn(BuiltinIrGenCtx, LocalId, LocalId, LocalId),
    },
    // TODO: 可変長
}

impl BuiltinConversionRule {
    pub fn ret_type(self) -> Type {
        match self {
            BuiltinConversionRule::Unary { ret, .. } => ret,
            BuiltinConversionRule::Binary { ret, .. } => ret,
            BuiltinConversionRule::Ternary { ret, .. } => ret,
        }
    }

    // TODO: 可変長引数が関わると返り値を変える必要あり
    pub fn arg_count(self) -> usize {
        self.arg_types().len()
    }

    pub fn arg_types(&self) -> &[Type] {
        match self {
            BuiltinConversionRule::Unary { args, .. } => args,
            BuiltinConversionRule::Binary { args, .. } => args,
            BuiltinConversionRule::Ternary { args, .. } => args,
        }
    }

    pub fn from_builtin(builtin: ast::Builtin) -> Vec<BuiltinConversionRule> {
        use ast::Builtin;

        match builtin {
            Builtin::Display => vec![BuiltinConversionRule::Unary {
                // TODO: 一旦Stringのみ
                args: [Type::Val(ValType::String)],
                ret: Type::Val(ValType::Nil),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Display(arg1),
                    });
                },
            }],
            Builtin::Add => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Int),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::AddInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Float),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::AddFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Sub => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Int),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::SubInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Float),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::SubFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Mul => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Int),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::MulInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Float),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::MulFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Div => vec![BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                ret: Type::Val(ValType::Float),
                ir_gen: |ctx, arg1, arg2| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::DivFloat(arg1, arg2),
                    });
                },
            }],
            Builtin::Quotient => vec![BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                ret: Type::Val(ValType::Int),
                ir_gen: |ctx, arg1, arg2| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::DivInt(arg1, arg2),
                    });
                },
            }],

            Builtin::WriteChar => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Char)],
                ret: Type::Val(ValType::Nil),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::WriteChar(arg1),
                    });
                },
            }],
            Builtin::IsPair => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::Cons, arg1),
                    });
                },
            }],
            Builtin::IsSymbol => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::Symbol, arg1),
                    });
                },
            }],
            Builtin::IsString => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::String, arg1),
                    });
                },
            }],
            Builtin::IsNumber => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    let is_int_local = ctx.locals.push_with(|id| Local {
                        id,
                        typ: ValType::Bool.into(),
                    });
                    let is_float_local = ctx.locals.push_with(|id| Local {
                        id,
                        typ: ValType::Bool.into(),
                    });
                    ctx.exprs.push(Instr {
                        local: Some(is_int_local),
                        kind: InstrKind::Is(ValType::Int, arg1),
                    });
                    ctx.exprs.push(Instr {
                        local: Some(is_float_local),
                        kind: InstrKind::Is(ValType::Float, arg1),
                    });
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Or(is_int_local, is_float_local),
                    });
                },
            }],
            Builtin::IsBoolean => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::Bool, arg1),
                    });
                },
            }],
            Builtin::IsProcedure => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::Closure, arg1),
                    });
                },
            }],
            Builtin::IsChar => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::Char, arg1),
                    });
                },
            }],
            Builtin::IsVector => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::Vector, arg1),
                    });
                },
            }],
            Builtin::VectorLength => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Vector)],
                ret: Type::Val(ValType::Int),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::VectorLength(arg1),
                    });
                },
            }],
            Builtin::VectorRef => vec![BuiltinConversionRule::Binary {
                args: [Type::Val(ValType::Vector), Type::Val(ValType::Int)],
                ret: Type::Obj,
                ir_gen: |ctx, arg1, arg2| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::VectorRef(arg1, arg2),
                    });
                },
            }],
            Builtin::VectorSet => vec![BuiltinConversionRule::Ternary {
                args: [
                    Type::Val(ValType::Vector),
                    Type::Val(ValType::Int),
                    Type::Obj,
                ],
                ret: Type::Val(ValType::Nil),
                ir_gen: |ctx, arg1, arg2, arg3| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::VectorSet(arg1, arg2, arg3),
                    });
                },
            }],
            Builtin::IsS64Vector => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::UVector(UVectorKind::S64), arg1),
                    });
                },
            }],
            Builtin::IsF64Vector => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Is(ValType::UVector(UVectorKind::F64), arg1),
                    });
                },
            }],
            Builtin::IsUVector => vec![BuiltinConversionRule::Unary {
                args: [Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1| {
                    let is_s64_local = ctx.locals.push_with(|id| Local {
                        id,
                        typ: ValType::Bool.into(),
                    });
                    let is_f64_local = ctx.locals.push_with(|id| Local {
                        id,
                        typ: ValType::Bool.into(),
                    });
                    ctx.exprs.push(Instr {
                        local: Some(is_s64_local),
                        kind: InstrKind::Is(ValType::UVector(UVectorKind::S64), arg1),
                    });
                    ctx.exprs.push(Instr {
                        local: Some(is_f64_local),
                        kind: InstrKind::Is(ValType::UVector(UVectorKind::F64), arg1),
                    });
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Or(is_s64_local, is_f64_local),
                    });
                },
            }],
            Builtin::MakeS64Vector => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Int)],
                ret: Type::Val(ValType::UVector(UVectorKind::S64)),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::MakeUVector(UVectorKind::S64, arg1),
                    });
                },
            }],
            Builtin::MakeF64Vector => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Int)],
                ret: Type::Val(ValType::UVector(UVectorKind::F64)),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::MakeUVector(UVectorKind::F64, arg1),
                    });
                },
            }],
            Builtin::UVectorLength => vec![
                BuiltinConversionRule::Unary {
                    args: [Type::Val(ValType::UVector(UVectorKind::S64))],
                    ret: Type::Val(ValType::Int),
                    ir_gen: |ctx, arg1| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::UVectorLength(UVectorKind::S64, arg1),
                        });
                    },
                },
                BuiltinConversionRule::Unary {
                    args: [Type::Val(ValType::UVector(UVectorKind::F64))],
                    ret: Type::Val(ValType::Int),
                    ir_gen: |ctx, arg1| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::UVectorLength(UVectorKind::F64, arg1),
                        });
                    },
                },
            ],
            Builtin::UVectorRef => vec![
                BuiltinConversionRule::Binary {
                    args: [
                        Type::Val(ValType::UVector(UVectorKind::S64)),
                        Type::Val(ValType::Int),
                    ],
                    ret: Type::Val(ValType::Int),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::UVectorRef(UVectorKind::S64, arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [
                        Type::Val(ValType::UVector(UVectorKind::F64)),
                        Type::Val(ValType::Int),
                    ],
                    ret: Type::Val(ValType::Float),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::UVectorRef(UVectorKind::F64, arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::UVectorSet => vec![
                BuiltinConversionRule::Ternary {
                    args: [
                        Type::Val(ValType::UVector(UVectorKind::S64)),
                        Type::Val(ValType::Int),
                        Type::Val(ValType::Int),
                    ],
                    ret: Type::Val(ValType::Nil),
                    ir_gen: |ctx, arg1, arg2, arg3| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::UVectorSet(UVectorKind::S64, arg1, arg2, arg3),
                        });
                    },
                },
                BuiltinConversionRule::Ternary {
                    args: [
                        Type::Val(ValType::UVector(UVectorKind::F64)),
                        Type::Val(ValType::Int),
                        Type::Val(ValType::Float),
                    ],
                    ret: Type::Val(ValType::Nil),
                    ir_gen: |ctx, arg1, arg2, arg3| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::UVectorSet(UVectorKind::F64, arg1, arg2, arg3),
                        });
                    },
                },
            ],
            Builtin::Eq => vec![BuiltinConversionRule::Binary {
                args: [Type::Obj, Type::Obj],
                ret: Type::Val(ValType::Bool),
                ir_gen: |ctx, arg1, arg2| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::EqObj(arg1, arg2),
                    });
                },
            }],
            Builtin::Cons => vec![BuiltinConversionRule::Binary {
                args: [Type::Obj, Type::Obj],
                ret: Type::Val(ValType::Cons),
                ir_gen: |ctx, arg1, arg2| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Cons(arg1, arg2),
                    });
                },
            }],
            Builtin::Car => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Cons)],
                ret: Type::Obj,
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Car(arg1),
                    });
                },
            }],
            Builtin::Cdr => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Cons)],
                ret: Type::Obj,
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::Cdr(arg1),
                    });
                },
            }],
            Builtin::SymbolToString => vec![BuiltinConversionRule::Unary {
                args: [Type::Val(ValType::Symbol)],
                ret: Type::Val(ValType::String),
                ir_gen: |ctx, arg1| {
                    ctx.exprs.push(Instr {
                        local: Some(ctx.dest),
                        kind: InstrKind::SymbolToString(arg1),
                    });
                },
            }],
            Builtin::NumberToString => vec![
                BuiltinConversionRule::Unary {
                    args: [Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::String),
                    ir_gen: |ctx, arg1| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::IntToString(arg1),
                        });
                    },
                },
                BuiltinConversionRule::Unary {
                    args: [Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::String),
                    ir_gen: |ctx, arg1| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::FloatToString(arg1),
                        });
                    },
                },
            ],
            Builtin::EqNum => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::EqInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::EqFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Lt => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::LtInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::LtFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Gt => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::GtInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::GtFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Le => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::LeInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::LeFloat(arg1, arg2),
                        });
                    },
                },
            ],
            Builtin::Ge => vec![
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Int), Type::Val(ValType::Int)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::GeInt(arg1, arg2),
                        });
                    },
                },
                BuiltinConversionRule::Binary {
                    args: [Type::Val(ValType::Float), Type::Val(ValType::Float)],
                    ret: Type::Val(ValType::Bool),
                    ir_gen: |ctx, arg1, arg2| {
                        ctx.exprs.push(Instr {
                            local: Some(ctx.dest),
                            kind: InstrKind::GeFloat(arg1, arg2),
                        });
                    },
                },
            ],
        }
    }
}
