use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use typed_index_collections::{TiSlice, ti_vec};

use crate::ir::{BasicBlockNext, BasicBlockTerminator};

use crate::ir;
use wasm_encoder::{
    BlockType, CodeSection, CompositeInnerType, CompositeType, ConstExpr, DataCountSection,
    DataSection, ElementSection, Elements, EntityType, ExportKind, ExportSection, FieldType,
    Function, FunctionSection, GlobalSection, GlobalType, HeapType, ImportSection, Instruction,
    MemoryType, Module, RefType, StorageType, StructType, SubType, TableSection, TableType,
    TypeSection, ValType,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WasmFuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

pub fn generate(module: &ir::Module) -> Vec<u8> {
    let module_gen = ModuleGenerator::new(module);
    let module = module_gen.generate();
    module.finish()
}

#[derive(Debug)]
struct ModuleGenerator<'a> {
    module: &'a ir::Module,
    type_count: u32,
    func_count: u32,
    global_count: u32,
    table_count: u32,
    // runtime functions
    instantiate_func_func: u32,
    instantiate_bb_func: u32,
    display_func: u32,
    display_fd_func: u32,
    string_to_symbol_func: u32,
    write_char_func: u32,
    int_to_string_func: u32,
    throw_webassembly_exception: u32,
    // wasm section
    imports: ImportSection,
    types: TypeSection,
    functions: FunctionSection,
    tables: TableSection,
    code: CodeSection,
    globals: GlobalSection,
    datas: DataSection,
    elements: ElementSection,
    func_indices: FxHashMap<ir::FuncId, u32>,
    exports: ExportSection,
    // types
    vector_inner_type: u32,
    mut_cell_types: FxHashMap<ir::Type, u32>,
    val_type: u32,
    nil_type: u32,
    bool_type: u32,
    int_type: u32,
    char_type: u32,
    cons_type: u32,
    buf_type: u32,
    string_buf_type: u32,
    string_type: u32,
    symbol_type: u32,
    vector_type: u32,
    closure_type: u32,
    closure_types: FxHashMap<Vec<ValType>, u32>, // env types -> type index
    func_types: FxHashMap<WasmFuncType, u32>,
    closure_type_fields: Vec<FieldType>,
    func_ref_type: u32,
    args_type: u32,
    // table
    global_table: u32,
    // const
    nil_global: Option<u32>,
    true_global: Option<u32>,
    false_global: Option<u32>,
    func_ref_globals: FxHashMap<ir::FuncId, u32>,
}

impl<'a> ModuleGenerator<'a> {
    fn new(module: &'a ir::Module) -> Self {
        Self {
            module,
            type_count: 0,
            func_count: 0,
            global_count: 0,
            instantiate_func_func: 0,
            instantiate_bb_func: 0,
            display_func: 0,
            display_fd_func: 0,
            string_to_symbol_func: 0,
            write_char_func: 0,
            int_to_string_func: 0,
            throw_webassembly_exception: 0,
            imports: ImportSection::new(),
            types: TypeSection::new(),
            functions: FunctionSection::new(),
            tables: TableSection::new(),
            code: CodeSection::new(),
            globals: GlobalSection::new(),
            elements: ElementSection::new(),
            func_indices: FxHashMap::default(),
            datas: DataSection::new(),
            exports: ExportSection::new(),
            mut_cell_types: FxHashMap::default(),
            vector_inner_type: 0,
            val_type: 0,
            nil_type: 0,
            bool_type: 0,
            int_type: 0,
            char_type: 0,
            cons_type: 0,
            buf_type: 0,
            string_buf_type: 0,
            string_type: 0,
            symbol_type: 0,
            vector_type: 0,
            closure_type: 0,
            closure_types: FxHashMap::default(),
            func_types: FxHashMap::default(),
            closure_type_fields: Vec::new(),
            func_ref_type: 0,
            args_type: 0,
            global_table: 0,
            nil_global: None,
            true_global: None,
            false_global: None,
            table_count: 0,
            func_ref_globals: FxHashMap::default(),
        }
    }

    fn add_runtime_function(&mut self, name: &str, func_type: WasmFuncType) -> u32 {
        let type_index = self.func_type(func_type);
        self.imports
            .import("runtime", name, EntityType::Function(type_index));
        let func_index = self.func_count;
        self.func_count += 1;
        func_index
    }

    fn func_type(&mut self, func_type: WasmFuncType) -> u32 {
        *self
            .func_types
            .entry(func_type)
            .or_insert_with_key(|func_type| {
                let type_index = self.type_count;
                self.type_count += 1;
                self.types
                    .ty()
                    .function(func_type.params.clone(), func_type.results.clone());
                type_index
            })
    }

    fn func_type_from_ir(&mut self, ir_func_type: &ir::FuncType) -> u32 {
        let params = ir_func_type
            .args
            .iter()
            .map(|ty| self.convert_local_type(ty))
            .collect();
        let results = vec![self.convert_local_type(&ir_func_type.ret)];
        self.func_type(WasmFuncType { params, results })
    }

    fn closure_type(&mut self, env_types: Vec<ValType>) -> u32 {
        *self
            .closure_types
            .entry(env_types)
            .or_insert_with_key(|env_types| {
                let mut fields = self.closure_type_fields.clone();
                for ty in env_types.iter() {
                    fields.push(FieldType {
                        element_type: StorageType::Val(*ty),
                        mutable: false,
                    });
                }

                let type_index = self.type_count;
                self.type_count += 1;

                self.types.ty().subtype(&SubType {
                    is_final: true,
                    supertype_idx: Some(self.closure_type),
                    composite_type: CompositeType {
                        shared: false,
                        inner: CompositeInnerType::Struct(StructType {
                            fields: fields.into_boxed_slice(),
                        }),
                    },
                });

                type_index
            })
    }

    fn closure_type_from_ir(&mut self, env_types: &[ir::LocalType]) -> u32 {
        let types = env_types
            .iter()
            .map(|ty| self.convert_local_type(ty))
            .collect();
        self.closure_type(types)
    }

    const VAL_TYPE_FIELDS: [FieldType; 1] = [FieldType {
        element_type: StorageType::I8,
        mutable: false,
    }];
    // const VAL_TAG_FIELD: u32 = 1;
    const BOOL_VALUE_FIELD: u32 = 1;
    const CHAR_VALUE_FIELD: u32 = 1;
    const INT_VALUE_FIELD: u32 = 1;
    // const STRING_BUF_FIELD: u32 = 1;
    // const STRING_LEN_FIELD: u32 = 2;
    // const STRING_OFFSET_FIELD: u32 = 3;
    const SYMBOL_STRING_FIELD: u32 = 1;
    const CONS_CAR_FIELD: u32 = 1;
    const CONS_CDR_FIELD: u32 = 2;
    const VECTOR_INNER_FIELD: u32 = 1;
    const FUNC_REF_FIELD_FUNC: u32 = 1;
    const CLOSURE_FUNC_FIELD: u32 = 1;
    const CLOSURE_ENVS_FIELD_OFFSET: u32 = 2;

    const MUT_CELL_VALUE_FIELD: u32 = 0;
    // const STRING_BUF_BUF_FIELD: u32 = 0;
    // const STRING_BUF_SHARED_FIELD: u32 = 1;

    pub fn generate(mut self) -> Module {
        self.val_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: false,
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: Self::VAL_TYPE_FIELDS.into(),
                }),
            },
        });

        self.vector_inner_type = self.type_count;
        self.type_count += 1;
        self.types.ty().array(
            &StorageType::Val(ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.val_type),
            })),
            true,
        );

        self.buf_type = self.type_count;
        self.type_count += 1;
        self.types.ty().array(&StorageType::I8, true);

        self.string_buf_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.buf_type),
                })),
                mutable: false,
            },
            FieldType {
                element_type: StorageType::I8,
                mutable: true,
            },
        ]);

        self.nil_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: Self::VAL_TYPE_FIELDS.into(),
                }),
            },
        });

        self.bool_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::I8,
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.char_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::I32),
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.int_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::I64),
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.string_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType {
                                nullable: false,
                                heap_type: HeapType::Concrete(self.string_buf_type),
                            })),
                            mutable: true,
                        });
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::I32),
                            mutable: false,
                        });
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::I32),
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.symbol_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType {
                                nullable: false,
                                heap_type: HeapType::Concrete(self.string_type),
                            })),
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.cons_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType {
                                nullable: true,
                                heap_type: HeapType::Concrete(self.val_type),
                            })),
                            mutable: true,
                        });
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType {
                                nullable: true,
                                heap_type: HeapType::Concrete(self.val_type),
                            })),
                            mutable: true,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.vector_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType {
                                nullable: false,
                                heap_type: HeapType::Concrete(self.vector_inner_type),
                            })),
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.func_ref_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::FUNCREF),
                            mutable: false,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.closure_type = self.type_count;
        self.type_count += 1;
        self.closure_type_fields = {
            let mut fields = Self::VAL_TYPE_FIELDS.to_vec();
            fields.push(FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.func_ref_type),
                })),
                mutable: false,
            });
            fields
        };
        self.types.ty().subtype(&SubType {
            is_final: false,
            supertype_idx: Some(self.val_type),
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: self.closure_type_fields.clone().into_boxed_slice(),
                }),
            },
        });

        self.args_type = self.type_count;
        self.type_count += 1;
        self.types.ty().array(
            &StorageType::Val(ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.val_type),
            })),
            true,
        );

        self.imports.import(
            "runtime",
            "memory",
            EntityType::Memory(MemoryType {
                minimum: 1,
                maximum: None,
                shared: false,
                memory64: false,
                page_size_log2: None,
            }),
        );

        self.nil_global = Some(self.global_count);
        self.imports.import(
            "runtime",
            "nil",
            EntityType::Global(GlobalType {
                val_type: ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.nil_type),
                }),
                mutable: false,
                shared: false,
            }),
        );
        self.global_count += 1;

        self.true_global = Some(self.global_count);
        self.imports.import(
            "runtime",
            "true",
            EntityType::Global(GlobalType {
                val_type: ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.bool_type),
                }),
                mutable: false,
                shared: false,
            }),
        );
        self.global_count += 1;

        self.false_global = Some(self.global_count);
        self.imports.import(
            "runtime",
            "false",
            EntityType::Global(GlobalType {
                val_type: ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.bool_type),
                }),
                mutable: false,
                shared: false,
            }),
        );
        self.global_count += 1;

        self.global_table = self.table_count;
        self.table_count += 1;
        self.imports.import(
            "runtime",
            "globals",
            EntityType::Table(TableType {
                element_type: RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.val_type),
                },
                table64: false,
                minimum: 0,
                maximum: None,
                shared: false,
            }),
        );

        self.instantiate_func_func = self.add_runtime_function("instantiate_func", WasmFuncType {
            params: vec![ValType::I32, ValType::I32],
            results: vec![ValType::I32],
        });
        self.instantiate_bb_func = self.add_runtime_function("instantiate_bb", WasmFuncType {
            params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            results: vec![ValType::I32],
        });

        self.display_func = self.add_runtime_function("display", WasmFuncType {
            params: vec![ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.string_type),
            })],
            results: vec![],
        });
        self.display_fd_func = self.add_runtime_function("display_fd", WasmFuncType {
            params: vec![
                ValType::I32,
                ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.string_type),
                }),
            ],
            results: vec![],
        });
        self.string_to_symbol_func = self.add_runtime_function("string_to_symbol", WasmFuncType {
            params: vec![ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.string_type),
            })],
            results: vec![ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.symbol_type),
            })],
        });

        self.write_char_func = self.add_runtime_function("write_char", WasmFuncType {
            params: vec![ValType::I32],
            results: vec![],
        });
        self.int_to_string_func = self.add_runtime_function("int_to_string", WasmFuncType {
            params: vec![ValType::I64],
            results: vec![ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.string_type),
            })],
        });

        self.throw_webassembly_exception =
            self.add_runtime_function("throw_webassembly_exception", WasmFuncType {
                params: vec![],
                results: vec![],
            });

        for func in self.module.funcs.iter() {
            let func_idx = self.func_count;
            self.func_count += 1;

            self.func_indices.insert(func.id, func_idx);
            self.elements
                .declared(Elements::Functions(Cow::Borrowed(&[func_idx])));

            self.func_ref_globals.insert(func.id, self.global_count);
            self.globals.global(
                GlobalType {
                    val_type: ValType::Ref(RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(self.func_ref_type),
                    }),
                    mutable: true,
                    shared: false,
                },
                &ConstExpr::ref_null(HeapType::Concrete(self.func_ref_type)),
            );
            self.global_count += 1;
        }
        for func in self.module.funcs.iter() {
            FuncGenerator::new(&mut self, func).gen_func();
        }

        self.exports.export(
            "start",
            ExportKind::Func,
            self.func_indices[&self.module.entry],
        );

        let mut module = Module::new();
        module
            .section(&self.types)
            .section(&self.imports)
            .section(&self.functions)
            .section(&self.tables)
            .section(&self.globals)
            .section(&self.exports)
            .section(&self.elements)
            .section(&DataCountSection {
                count: self.datas.len(),
            })
            .section(&self.code)
            .section(&self.datas);

        module
    }

    fn global_id_to_idx(&mut self, global: ir::GlobalId) -> i32 {
        debug_assert!(self.module.globals.contains(&global));

        usize::from(global) as i32
    }

    fn mut_cell_type(&mut self, inner_ty: &ir::Type) -> u32 {
        let ty = self.convert_type(inner_ty);

        *self.mut_cell_types.entry(*inner_ty).or_insert_with(|| {
            let type_id = self.type_count;
            self.type_count += 1;
            self.types.ty().struct_(vec![FieldType {
                element_type: StorageType::Val(ty),
                mutable: true,
            }]);
            type_id
        })
    }

    fn convert_local_type(&mut self, ty: &ir::LocalType) -> ValType {
        match ty {
            ir::LocalType::MutCell(inner_ty) => {
                let mut_cell_type = self.mut_cell_type(inner_ty);
                ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(mut_cell_type),
                })
            }
            ir::LocalType::Type(ty) => self.convert_type(ty),
            ir::LocalType::Args => ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.args_type),
            }),
        }
    }

    fn convert_type(&self, ty: &ir::Type) -> ValType {
        match ty {
            ir::Type::Boxed => ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.val_type),
            }),
            ir::Type::Val(val) => match val {
                ir::ValType::Bool => ValType::I32,
                ir::ValType::Int => ValType::I64,
                ir::ValType::Char => ValType::I32,
                ir::ValType::String => ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.string_type),
                }),
                ir::ValType::Symbol => ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.symbol_type),
                }),
                ir::ValType::Nil => ValType::I32,
                ir::ValType::Cons => ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.cons_type),
                }),
                ir::ValType::Closure => ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.closure_type),
                }),
                ir::ValType::Vector => ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.vector_type),
                }),
                ir::ValType::FuncRef => ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.func_ref_type),
                }),
            },
        }
    }
}

#[derive(Debug)]
enum StructuredBasicBlock {
    Simple(ir::BasicBlockId),
    If {
        cond: ir::LocalId,
        then: Vec<StructuredBasicBlock>,
        else_: Vec<StructuredBasicBlock>,
    },
    Terminator(ir::BasicBlockTerminator),
}

// relooper algorithmのような動作をするもの
// 閉路がないので単純
fn reloop(
    entry: ir::BasicBlockId,
    bbs: &TiSlice<ir::BasicBlockId, ir::BasicBlock>,
) -> Vec<StructuredBasicBlock> {
    let mut preds = ti_vec![FxHashSet::default(); bbs.len()];
    for bb in bbs.iter() {
        for next in bb.next.successors() {
            preds[next].insert(bb.id);
        }
    }

    let all_ids = FxHashSet::from_iter(bbs.iter().map(|bb| bb.id));
    let mut doms = ti_vec![all_ids.clone(); bbs.len()];
    doms[entry] = FxHashSet::from_iter(vec![entry]);
    let mut changed = true;
    while changed {
        changed = false;
        for bb in bbs.iter() {
            if bb.id == entry {
                continue;
            }
            let mut new_dom = all_ids.clone();
            for pred in preds[bb.id].iter() {
                new_dom.retain(|id| doms[*pred].contains(id));
            }
            new_dom.insert(bb.id);
            if new_dom.len() != doms[bb.id].len() {
                doms[bb.id] = new_dom;
                changed = true;
            }
        }
    }

    let mut reversed_doms = ti_vec![FxHashSet::default(); bbs.len()];
    for bb in bbs.iter() {
        for dom in doms[bb.id].iter() {
            reversed_doms[*dom].insert(bb.id);
        }
    }

    let mut results = Vec::new();
    let rejoin_point = reloop_rec(
        entry,
        bbs,
        &reversed_doms,
        &FxHashSet::default(),
        &mut results,
    );
    assert_eq!(rejoin_point, None);
    results
}

// rejoin pointを返す
fn reloop_rec(
    cur: ir::BasicBlockId,
    bbs: &TiSlice<ir::BasicBlockId, ir::BasicBlock>,
    reversed_doms: &TiSlice<ir::BasicBlockId, FxHashSet<ir::BasicBlockId>>,
    rejoin_points: &FxHashSet<ir::BasicBlockId>,
    results: &mut Vec<StructuredBasicBlock>,
) -> Option<ir::BasicBlockId> {
    if rejoin_points.contains(&cur) {
        return Some(cur);
    }
    results.push(StructuredBasicBlock::Simple(cur));
    let bb = &bbs[cur];
    // TODO: cloneを避ける
    match bb.next.clone() {
        BasicBlockNext::If(cond, then_target, else_target) => {
            let mut if_rejoin_points = reversed_doms[cur].clone();
            if_rejoin_points.retain(|bb_id| {
                !reversed_doms[then_target].contains(bb_id)
                    && !reversed_doms[else_target].contains(bb_id)
            });

            let mut then_bbs = Vec::new();
            let mut else_bbs = Vec::new();
            let rejoin_point1 = reloop_rec(
                then_target,
                bbs,
                reversed_doms,
                &if_rejoin_points,
                &mut then_bbs,
            );
            let rejoin_point2 = reloop_rec(
                else_target,
                bbs,
                reversed_doms,
                &if_rejoin_points,
                &mut else_bbs,
            );
            // 片方の分岐がtail cailの場合、片方だけNoneになる
            let rejoin_point = match (rejoin_point1, rejoin_point2) {
                (Some(p1), Some(p2)) => {
                    assert_eq!(p1, p2);
                    Some(p1)
                }
                (Some(p), None) | (None, Some(p)) => Some(p),
                (None, None) => None,
            };
            results.push(StructuredBasicBlock::If {
                cond,
                then: then_bbs,
                else_: else_bbs,
            });
            if let Some(rejoin_point) = rejoin_point {
                reloop_rec(rejoin_point, bbs, reversed_doms, rejoin_points, results)
            } else {
                None
            }
        }
        BasicBlockNext::Jump(target) => {
            reloop_rec(target, bbs, reversed_doms, rejoin_points, results)
        }
        BasicBlockNext::Terminator(terminator) => {
            results.push(StructuredBasicBlock::Terminator(terminator));
            None
        }
    }
}

#[derive(Debug)]
struct FuncGenerator<'a, 'b> {
    module_generator: &'a mut ModuleGenerator<'b>,
    func: &'a ir::Func,
    local_count: u32,
    local_ids: FxHashMap<ir::LocalId, u32>,
}

impl<'a, 'b> FuncGenerator<'a, 'b> {
    fn new(module_generator: &'a mut ModuleGenerator<'b>, func: &'a ir::Func) -> Self {
        Self {
            module_generator,
            func,
            local_count: 0,
            local_ids: FxHashMap::default(),
        }
    }

    fn local_id_to_idx(&mut self, local: ir::LocalId) -> u32 {
        *self.local_ids.get(&local).unwrap()
    }

    fn gen_func(mut self) {
        let mut is_args = ti_vec![false; self.func.locals.len()];
        for arg in &self.func.args {
            is_args[*arg] = true;
        }

        for local_id in
            // ローカル変数をargs, argsでない変数の順に並べる
            self.func.args.iter().copied().chain(
                self.func
                    .locals
                    .keys()
                    .filter(|local_id| !is_args[*local_id]),
            )
        {
            let idx = self.local_count;
            self.local_ids.insert(local_id, idx);
            self.local_count += 1;
        }

        let type_idx = self
            .module_generator
            .func_type_from_ir(&self.func.func_type());

        // TODO: ここでローカル変数を定義していたらgen_exprでローカル変数を追加できない
        let mut function = Function::new(
            self.func
                .locals
                .iter_enumerated()
                .filter(|(local_id, _)| !is_args[*local_id])
                .map(|(_, ty)| {
                    let ty = self.module_generator.convert_local_type(ty);
                    (1, ty)
                })
                .collect::<Vec<_>>(),
        );

        let structured_bbs = reloop(self.func.bb_entry, &self.func.bbs);
        for structured_bb in &structured_bbs {
            self.gen_bb(&mut function, self.func, structured_bb);
        }

        function.instruction(&Instruction::Unreachable); // TODO: 型チェックを通すため
        function.instruction(&Instruction::End);

        self.module_generator.functions.function(type_idx);
        self.module_generator.code.function(&function);
    }

    fn gen_bb(
        &mut self,
        function: &mut Function,
        func: &ir::Func,
        structured_bb: &StructuredBasicBlock,
    ) {
        match structured_bb {
            StructuredBasicBlock::Simple(bb_id) => {
                let bb = &func.bbs[*bb_id];
                for expr in &bb.exprs {
                    self.gen_assign(function, &func.locals, expr);
                }
            }
            StructuredBasicBlock::If { cond, then, else_ } => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*cond)));
                function.instruction(&Instruction::If(BlockType::Empty));
                for structured_bb in then {
                    self.gen_bb(function, func, structured_bb);
                }
                function.instruction(&Instruction::Else);
                for structured_bb in else_ {
                    self.gen_bb(function, func, structured_bb);
                }
                function.instruction(&Instruction::End);
            }
            StructuredBasicBlock::Terminator(terminator) => match terminator {
                BasicBlockTerminator::Return(local) => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*local)));
                    function.instruction(&Instruction::Return);
                }
                BasicBlockTerminator::TailCall(call) => {
                    self.gen_call(function, true, call);
                }
                BasicBlockTerminator::TailCallRef(call_ref) => {
                    self.gen_call_ref(function, true, call_ref);
                }
            },
        }
    }

    fn gen_assign(
        &mut self,
        function: &mut Function,
        locals: &TiSlice<ir::LocalId, ir::LocalType>,
        expr: &ir::ExprAssign,
    ) {
        if let ir::Expr::Nop = expr.expr {
            return;
        }
        self.gen_expr(function, locals, &expr.expr);
        if let Some(local) = &expr.local {
            function.instruction(&Instruction::LocalSet(self.local_id_to_idx(*local)));
        } else {
            function.instruction(&Instruction::Drop);
        }
    }

    fn gen_expr(
        &mut self,
        function: &mut Function,
        locals: &TiSlice<ir::LocalId, ir::LocalType>,
        expr: &ir::Expr,
    ) {
        match expr {
            ir::Expr::Nop => {
                // unreachableなはず
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::InstantiateFunc(module_id, func_id) => {
                function.instruction(&Instruction::I32Const(usize::from(*module_id) as i32));
                function.instruction(&Instruction::I32Const(usize::from(*func_id) as i32));
                function.instruction(&Instruction::Call(
                    self.module_generator.instantiate_func_func,
                ));
            }
            ir::Expr::InstantiateBB(module_id, func_id, bb_id, index_local) => {
                function.instruction(&Instruction::I32Const(usize::from(*module_id) as i32));
                function.instruction(&Instruction::I32Const(usize::from(*func_id) as i32));
                function.instruction(&Instruction::I32Const(usize::from(*bb_id) as i32));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*index_local)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::Call(
                    self.module_generator.instantiate_bb_func,
                ));
            }
            ir::Expr::Bool(b) => {
                function.instruction(&Instruction::I32Const(if *b { 1 } else { 0 }));
            }
            ir::Expr::Int(i) => {
                function.instruction(&Instruction::I64Const(*i));
            }
            ir::Expr::Char(c) => {
                function.instruction(&Instruction::I32Const(*c as i32));
            }
            ir::Expr::String(s) => {
                // TODO: 重複リテラルを共有
                let bs = s.as_bytes();
                let data_index = self.module_generator.datas.len();
                self.module_generator.datas.passive(bs.iter().copied());

                // String.tag
                function.instruction(&Instruction::I32Const(ir::ValType::String.tag()));

                // data offset
                function.instruction(&Instruction::I32Const(0));
                // data len
                function.instruction(&Instruction::I32Const(bs.len() as i32));
                // StringBuf.buf
                function.instruction(&Instruction::ArrayNewData {
                    array_type_index: self.module_generator.buf_type,
                    array_data_index: data_index,
                });

                // StringBuf.shared
                function.instruction(&Instruction::I32Const(0));
                // String.buf
                function.instruction(&Instruction::StructNew(
                    self.module_generator.string_buf_type,
                ));

                // String.len
                function.instruction(&Instruction::I32Const(bs.len() as i32));
                // String.offset
                function.instruction(&Instruction::I32Const(0));
                function.instruction(&Instruction::StructNew(self.module_generator.string_type));
            }
            ir::Expr::StringToSymbol(s) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*s)));
                function.instruction(&Instruction::Call(
                    self.module_generator.string_to_symbol_func,
                ));
            }
            ir::Expr::Nil => {
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::Cons(car, cdr) => {
                function.instruction(&Instruction::I32Const(ir::ValType::Cons.tag()));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*car)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*cdr)));
                function.instruction(&Instruction::StructNew(self.module_generator.cons_type));
            }
            ir::Expr::Vector(vec) => {
                function.instruction(&Instruction::I32Const(ir::ValType::Vector.tag()));
                for elem in vec.iter() {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*elem)));
                }
                function.instruction(&Instruction::ArrayNewFixed {
                    array_type_index: self.module_generator.vector_inner_type,
                    array_size: vec.len() as u32,
                });
                function.instruction(&Instruction::StructNew(self.module_generator.vector_type));
            }
            ir::Expr::CreateMutCell(typ) => {
                function.instruction(&Instruction::RefNull(HeapType::Concrete(
                    self.module_generator.val_type,
                )));
                function.instruction(&Instruction::StructNew(
                    self.module_generator.mut_cell_type(typ),
                ));
            }
            ir::Expr::DerefMutCell(typ, cell) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*cell)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.mut_cell_type(typ),
                    field_index: ModuleGenerator::MUT_CELL_VALUE_FIELD,
                });
            }
            ir::Expr::SetMutCell(typ, cell, val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*cell)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::StructSet {
                    struct_type_index: self.module_generator.mut_cell_type(typ),
                    field_index: ModuleGenerator::MUT_CELL_VALUE_FIELD,
                });
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
            }
            ir::Expr::FuncRef(func) => {
                let global_idx = self.module_generator.func_ref_globals[func];
                function.instruction(&Instruction::GlobalGet(global_idx));
                function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                    self.module_generator.func_ref_type,
                )));
            }
            ir::Expr::Closure { envs, func } => {
                function.instruction(&Instruction::I32Const(ir::ValType::Closure.tag()));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*func)));
                for env in envs.iter() {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*env)));
                }

                function.instruction(&Instruction::StructNew(
                    self.module_generator.closure_type_from_ir(
                        &envs.iter().map(|env| locals[*env]).collect::<Vec<_>>(),
                    ),
                ));
            }
            ir::Expr::CallRef(call_ref) => {
                self.gen_call_ref(function, false, call_ref);
            }
            ir::Expr::ClosureFuncRef(closure) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*closure)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.closure_type,
                    field_index: ModuleGenerator::CLOSURE_FUNC_FIELD,
                });
            }
            ir::Expr::Call(call) => {
                self.gen_call(function, false, call);
            }
            ir::Expr::Move(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
            }
            ir::Expr::Unbox(typ, val) => match typ {
                ir::ValType::Bool => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.bool_type,
                    )));
                    function.instruction(&Instruction::StructGetU {
                        struct_type_index: self.module_generator.bool_type,
                        field_index: ModuleGenerator::BOOL_VALUE_FIELD,
                    });
                }
                ir::ValType::Int => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.int_type,
                    )));
                    function.instruction(&Instruction::StructGet {
                        struct_type_index: self.module_generator.int_type,
                        field_index: ModuleGenerator::INT_VALUE_FIELD,
                    });
                }
                ir::ValType::Char => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.char_type,
                    )));
                    function.instruction(&Instruction::StructGet {
                        struct_type_index: self.module_generator.char_type,
                        field_index: ModuleGenerator::CHAR_VALUE_FIELD,
                    });
                }
                ir::ValType::String => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.string_type,
                    )));
                }
                ir::ValType::Symbol => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.symbol_type,
                    )));
                }
                ir::ValType::Nil => {
                    // TODO: 型チェックするべき
                    function.instruction(&Instruction::I32Const(0));
                }
                ir::ValType::Cons => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.cons_type,
                    )));
                }
                ir::ValType::Closure => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.closure_type,
                    )));
                }
                ir::ValType::Vector => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.vector_type,
                    )));
                }
                ir::ValType::FuncRef => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.module_generator.func_ref_type,
                    )));
                }
            },
            ir::Expr::Box(typ, val) => match typ {
                ir::ValType::Bool => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::If(BlockType::Result(ValType::Ref(
                        RefType {
                            nullable: false,
                            heap_type: HeapType::Concrete(self.module_generator.bool_type),
                        },
                    ))));
                    function.instruction(&Instruction::GlobalGet(
                        self.module_generator.true_global.unwrap(),
                    ));
                    function.instruction(&Instruction::Else);
                    function.instruction(&Instruction::GlobalGet(
                        self.module_generator.false_global.unwrap(),
                    ));
                    function.instruction(&Instruction::End);
                }
                ir::ValType::Int => {
                    function.instruction(&Instruction::I32Const(ir::ValType::Int.tag()));
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::StructNew(self.module_generator.int_type));
                }
                ir::ValType::Char => {
                    function.instruction(&Instruction::I32Const(ir::ValType::Char.tag()));
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::StructNew(self.module_generator.char_type));
                }
                ir::ValType::String => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                }
                ir::ValType::Symbol => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                }
                ir::ValType::Nil => {
                    function.instruction(&Instruction::GlobalGet(
                        self.module_generator.nil_global.unwrap(),
                    ));
                }
                ir::ValType::Cons => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                }
                ir::ValType::Closure => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                }
                ir::ValType::Vector => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                }
                ir::ValType::FuncRef => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                }
            },
            ir::Expr::ClosureEnv(env_types, closure, env_index) => {
                let closure_type = self.module_generator.closure_type_from_ir(env_types);
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*closure)));
                // TODO: irでキャストするべき
                function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                    closure_type,
                )));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: closure_type,
                    field_index: ModuleGenerator::CLOSURE_ENVS_FIELD_OFFSET + *env_index as u32,
                });
            }
            ir::Expr::GlobalGet(global) => {
                function.instruction(&Instruction::I32Const(
                    self.module_generator.global_id_to_idx(*global),
                ));
                function.instruction(&Instruction::TableGet(self.module_generator.global_table));
            }
            ir::Expr::GlobalSet(global, val) => {
                function.instruction(&Instruction::I32Const(
                    self.module_generator.global_id_to_idx(*global),
                ));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::TableSet(self.module_generator.global_table));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
            }
            ir::Expr::Error(msg) => {
                function.instruction(&Instruction::I32Const(2));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*msg)));
                function.instruction(&Instruction::Call(self.module_generator.display_fd_func));
                function.instruction(&Instruction::Call(
                    self.module_generator.throw_webassembly_exception,
                ));
                // これがないとこの後のdropでコンパイルエラーになる
                function.instruction(&Instruction::RefNull(HeapType::Concrete(
                    self.module_generator.val_type,
                )));
            }
            ir::Expr::Display(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::Call(self.module_generator.display_func));
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::Add(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64Add);
            }
            ir::Expr::Sub(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64Sub);
            }
            ir::Expr::Mul(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64Mul);
            }
            ir::Expr::Div(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64DivS);
            }
            ir::Expr::WriteChar(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::Call(self.module_generator.write_char_func));
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::IsPair(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.cons_type,
                )));
            }
            ir::Expr::IsSymbol(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.symbol_type,
                )));
            }
            ir::Expr::IsString(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.string_type,
                )));
            }
            ir::Expr::IsChar(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.char_type,
                )));
            }
            ir::Expr::IsNumber(val) => {
                // TODO: 一般のnumberかを判定
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.int_type,
                )));
            }
            ir::Expr::IsBoolean(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.bool_type,
                )));
            }
            ir::Expr::IsProcedure(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.closure_type,
                )));
            }
            ir::Expr::IsVector(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.module_generator.vector_type,
                )));
            }
            ir::Expr::VectorLength(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.vector_type,
                    field_index: ModuleGenerator::VECTOR_INNER_FIELD,
                });
                function.instruction(&Instruction::ArrayLen);
                function.instruction(&Instruction::I64ExtendI32U);
            }
            ir::Expr::VectorRef(vector, index) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*vector)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.vector_type,
                    field_index: ModuleGenerator::VECTOR_INNER_FIELD,
                });
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*index)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::ArrayGet(
                    self.module_generator.vector_inner_type,
                ));
            }
            ir::Expr::VectorSet(vector, index, val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*vector)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.vector_type,
                    field_index: ModuleGenerator::VECTOR_INNER_FIELD,
                });
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*index)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::ArraySet(
                    self.module_generator.vector_inner_type,
                ));
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::Eq(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::RefEq);
            }
            ir::Expr::Not(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::I32Const(1));
                function.instruction(&Instruction::I32Xor);
            }
            ir::Expr::Car(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.cons_type,
                    field_index: ModuleGenerator::CONS_CAR_FIELD,
                });
            }
            ir::Expr::Cdr(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.cons_type,
                    field_index: ModuleGenerator::CONS_CDR_FIELD,
                });
            }
            ir::Expr::SymbolToString(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.symbol_type,
                    field_index: ModuleGenerator::SYMBOL_STRING_FIELD,
                });
            }
            ir::Expr::NumberToString(val) => {
                // TODO: 一般のnumberに対応
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::Call(self.module_generator.int_to_string_func));
            }
            ir::Expr::EqNum(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64Eq);
            }
            ir::Expr::Lt(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64LtS);
            }
            ir::Expr::Gt(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64GtS);
            }
            ir::Expr::Le(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64LeS);
            }
            ir::Expr::Ge(lhs, rhs) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*lhs)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*rhs)));
                function.instruction(&Instruction::I64GeS);
            }
            ir::Expr::Args(args) => {
                for arg in args.iter() {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*arg)));
                }
                function.instruction(&Instruction::ArrayNewFixed {
                    array_type_index: self.module_generator.args_type,
                    array_size: args.len() as u32,
                });
            }
            ir::Expr::ArgsRef(args, idx) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*args)));
                function.instruction(&Instruction::I32Const(*idx as i32));
                function.instruction(&Instruction::ArrayGet(self.module_generator.args_type));
            }

            ir::Expr::InitModule => {
                for func in self.module_generator.module.funcs.iter() {
                    let global_idx = self.module_generator.func_ref_globals[&func.id];
                    let func_idx = self.module_generator.func_indices[&func.id];
                    function.instruction(&Instruction::I32Const(ir::ValType::FuncRef.tag()));
                    function.instruction(&Instruction::RefFunc(func_idx));
                    function
                        .instruction(&Instruction::StructNew(self.module_generator.func_ref_type));
                    function.instruction(&Instruction::GlobalSet(global_idx));
                }
                // init globals
                let global_count = self
                    .module_generator
                    .module
                    .globals
                    .iter()
                    .map(|&id| usize::from(id) + 1)
                    .max()
                    .unwrap_or(0);

                // 必要なサイズになるまで2倍に拡張
                function.instruction(&Instruction::Block(BlockType::Empty));
                function.instruction(&Instruction::Loop(BlockType::Empty));
                function.instruction(&Instruction::TableSize(self.module_generator.global_table));
                function.instruction(&Instruction::I32Const(global_count as i32));
                function.instruction(&Instruction::I32GeU);
                function.instruction(&Instruction::BrIf(1));
                function.instruction(&Instruction::RefNull(HeapType::Concrete(
                    self.module_generator.val_type,
                )));
                function.instruction(&Instruction::TableSize(self.module_generator.global_table));
                function.instruction(&Instruction::TableGrow(self.module_generator.global_table));
                function.instruction(&Instruction::I32Const(-1));
                function.instruction(&Instruction::I32Eq);
                function.instruction(&Instruction::If(BlockType::Empty));
                function.instruction(&Instruction::Unreachable);
                function.instruction(&Instruction::End);
                function.instruction(&Instruction::Br(0));
                function.instruction(&Instruction::End);
                function.instruction(&Instruction::End);

                function.instruction(&Instruction::GlobalGet(
                    self.module_generator.nil_global.unwrap(),
                ));
            }
        }
    }

    fn gen_call_ref(&mut self, function: &mut Function, is_tail: bool, call_ref: &ir::ExprCallRef) {
        let func_type = self.module_generator.func_type_from_ir(&call_ref.func_type);

        for arg in &call_ref.args {
            function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*arg)));
        }

        function.instruction(&Instruction::LocalGet(self.local_id_to_idx(call_ref.func)));
        function.instruction(&Instruction::StructGet {
            struct_type_index: self.module_generator.func_ref_type,
            field_index: ModuleGenerator::FUNC_REF_FIELD_FUNC,
        });
        function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(func_type)));
        if is_tail {
            function.instruction(&Instruction::ReturnCallRef(func_type));
        } else {
            function.instruction(&Instruction::CallRef(func_type));
        }
    }

    fn gen_call(&mut self, function: &mut Function, is_tail: bool, call: &ir::ExprCall) {
        let func_idx = self.module_generator.func_indices[&call.func_id];
        for arg in &call.args {
            function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*arg)));
        }
        if is_tail {
            function.instruction(&Instruction::ReturnCall(func_idx));
        } else {
            function.instruction(&Instruction::Call(func_idx));
        }
    }
}
