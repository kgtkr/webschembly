use rustc_hash::FxHashMap;

use super::global_layout::GLOBAL_LAYOUT_MAX_SIZE;
use super::jit_ctx::JitCtx;
use super::jit_func::JitSpecializedFunc;
use crate::ir_generator::GlobalManager;
use vec_map::{HasId, VecMap};
use webschembly_compiler_ir::*;
#[derive(Debug)]
pub struct JitModule {
    module_id: ModuleId,
    module: Module,
    jit_funcs: FxHashMap<(FuncId, usize), JitSpecializedFunc>,
    func_to_globals: VecMap<FuncId, GlobalId>,
    func_types: VecMap<FuncId, FuncType>,
}

impl HasId for JitModule {
    type Id = ModuleId;
    fn id(&self) -> Self::Id {
        self.module_id
    }
}

impl JitModule {
    pub fn new(global_manager: &mut GlobalManager, module_id: ModuleId, module: Module) -> Self {
        let func_to_globals = module
            .funcs
            .keys()
            .map(|id| (id, global_manager.gen_global(LocalType::FuncRef).id))
            .collect::<VecMap<FuncId, _>>();

        let func_types = module
            .funcs
            .iter()
            .map(|(id, f)| (id, f.func_type()))
            .collect::<VecMap<FuncId, _>>();

        Self {
            module_id,
            module,
            jit_funcs: FxHashMap::default(),
            func_to_globals,
            func_types,
        }
    }

    pub fn generate_stub_module(
        &self,
        global_manager: &mut GlobalManager,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        // entry関数もあるので+1してる
        let mut stub_func_ids = FxHashMap::default();
        let mut funcs = VecMap::new();

        /*
        以下のようなentryを生成
        func entry() {
            set_global f0_ref f0_stub
            set_global f1_ref f1_stub

            f0_stub()
        }
        */

        for func in self.module.funcs.values() {
            /*
            以下のようなスタブを生成
            func f0_stub(x1, x2) {
                instantiate_module(f0_module);
                f0 <- get_global f0_ref
                f0(x1, x2)
            }
            */
            let mut new_locals = func.locals.clone();
            let f0_ref_local = new_locals.push_with(|id| Local {
                id,
                typ: LocalType::FuncRef,
            });

            let id = funcs.push_with(|id| Func {
                id,
                args: func.args.clone(),
                ret_type: func.ret_type,
                locals: new_locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: vec![
                        Instr {
                            local: None,
                            kind: InstrKind::InstantiateFunc(self.module_id, func.id, 0),
                        },
                        Instr {
                            local: Some(f0_ref_local),
                            kind: InstrKind::GlobalGet(self.func_to_globals[func.id]),
                        },
                    ],
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCallRef(
                        InstrCallRef {
                            func: f0_ref_local,
                            args: func.args.clone(),
                            func_type: func.func_type(),
                        },
                    )),
                }]
                .into_iter()
                .collect(),
            });
            stub_func_ids.insert(func.id, id);
        }

        let entry_func_id = {
            // entry
            let mut locals = VecMap::new();
            let mut exprs = Vec::new();
            for func in self.module.funcs.values() {
                let func_ref_local = locals.push_with(|id| Local {
                    id,
                    typ: LocalType::FuncRef,
                });

                exprs.push(Instr {
                    local: Some(func_ref_local),
                    kind: InstrKind::FuncRef(stub_func_ids[&func.id]),
                });
                exprs.push(Instr {
                    local: None,
                    kind: InstrKind::GlobalSet(self.func_to_globals[func.id], func_ref_local),
                });
            }

            // 最初にインスタンス化されるモジュールなら初期化処理
            if !jit_ctx.is_instantiated() {
                let mut stub_globals = FxHashMap::default();
                for func_index in 0..GLOBAL_LAYOUT_MAX_SIZE {
                    let stub_global = global_manager.gen_global(LocalType::MutFuncRef);
                    stub_globals.insert(func_index, stub_global);
                    let stub_local = locals.push_with(|id| Local {
                        id,
                        typ: LocalType::MutFuncRef,
                    });
                    exprs.push(Instr {
                        local: Some(stub_local),
                        kind: InstrKind::CreateEmptyMutFuncRef,
                    });
                    exprs.push(Instr {
                        local: None,
                        kind: InstrKind::GlobalSet(stub_global.id, stub_local),
                    });
                }

                let instantiate_func_global = global_manager.gen_global(LocalType::FuncRef);
                jit_ctx.init_instantiated(stub_globals, instantiate_func_global);
            };

            funcs.push_with(|id| Func {
                id,
                args: vec![],
                ret_type: LocalType::Type(Type::Obj),
                locals,
                bb_entry: BasicBlockId::from(0),
                bbs: [BasicBlock {
                    id: BasicBlockId::from(0),
                    instrs: exprs,
                    next: BasicBlockNext::Terminator(BasicBlockTerminator::TailCall(InstrCall {
                        func_id: stub_func_ids[&self.module.entry],
                        args: vec![],
                    })),
                }]
                .into_iter()
                .collect(),
            })
        };
        Module {
            globals: FxHashMap::default(),
            funcs,
            entry: entry_func_id,
            meta: Meta {
                // TODO:
                local_metas: FxHashMap::default(),
                global_metas: FxHashMap::default(),
            },
        }
    }

    pub fn instantiate_func(
        &mut self,
        global_manager: &mut GlobalManager,
        func_id: FuncId,
        func_index: usize,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        let jit_func = JitSpecializedFunc::new(
            self.module_id,
            global_manager,
            &self.module.funcs[func_id],
            func_index,
            jit_ctx,
        );
        self.jit_funcs.insert((func_id, func_index), jit_func);

        self.jit_funcs
            .get_mut(&(func_id, func_index))
            .unwrap()
            .generate_func_module(
                &self.func_to_globals,
                &self.func_types,
                global_manager,
                jit_ctx,
            )
    }

    pub fn instantiate_bb(
        &mut self,
        func_id: FuncId,
        func_index: usize,
        bb_id: BasicBlockId,
        index: usize,
        global_manager: &mut GlobalManager,
        jit_ctx: &mut JitCtx,
    ) -> Module {
        let jit_func = self.jit_funcs.get_mut(&(func_id, func_index)).unwrap();
        let (module, _) = jit_func.generate_bb_module(
            &self.func_to_globals,
            &self.func_types,
            bb_id,
            index,
            global_manager,
            jit_ctx,
        );
        module
    }
}
