use rustc_hash::FxHashMap;
use std::borrow::Cow;

use crate::ir::BasicBlockTerminator;

use crate::wasm_generator::relooper::{Structured, reloop};
use crate::{VecMap, ir};
use wasm_encoder::{
    AbstractHeapType, BlockType, CodeSection, CompositeInnerType, CompositeType, ConstExpr,
    DataCountSection, DataSection, ElementSection, Elements, EntityType, ExportKind, ExportSection,
    FieldType, Function, FunctionSection, GlobalSection, GlobalType, HeapType, ImportSection,
    Instruction, MemoryType, Module, RefType, StorageType, StructType, SubType, TableSection,
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
    ref_types: FxHashMap<ir::Type, u32>,
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
    args_type: u32,
    mut_func_ref_type: u32,
    entrypoint_table_type: u32,
    global_id_to_idx: FxHashMap<ir::GlobalId, u32>,
    // const
    nil_global: Option<u32>,
    true_global: Option<u32>,
    false_global: Option<u32>,
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
            ref_types: FxHashMap::default(),
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
            args_type: 0,
            mut_func_ref_type: 0,
            entrypoint_table_type: 0,
            global_id_to_idx: FxHashMap::default(),
            nil_global: None,
            true_global: None,
            false_global: None,
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
            .map(|&ty| self.convert_local_type(ty))
            .collect();
        let results = vec![self.convert_local_type(ir_func_type.ret)];
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
            .map(|&ty| self.convert_local_type(ty))
            .collect();
        self.closure_type(types)
    }

    const BOOL_VALUE_FIELD: u32 = 0;
    const CHAR_VALUE_FIELD: u32 = 0;
    const INT_VALUE_FIELD: u32 = 0;
    // const STRING_BUF_FIELD: u32 = 0;
    // const STRING_LEN_FIELD: u32 = 1;
    // const STRING_OFFSET_FIELD: u32 = 2;
    const SYMBOL_STRING_FIELD: u32 = 0;
    const CONS_CAR_FIELD: u32 = 0;
    const CONS_CDR_FIELD: u32 = 1;
    const CLOSURE_MODULE_ID_FIELD: u32 = 0;
    const CLOSURE_FUNC_ID_FIELD: u32 = 1;
    const CLOSURE_ENTRYPOINT_TABLE_FIELD: u32 = 2;
    const CLOSURE_ENVS_FIELD_OFFSET: u32 = 3;

    const REF_VALUE_FIELD: u32 = 0;
    // const STRING_BUF_BUF_FIELD: u32 = 0;
    // const STRING_BUF_SHARED_FIELD: u32 = 1;

    const MUT_FUNC_REF_FUNC_FIELD: u32 = 0;

    pub fn generate(mut self) -> Module {
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
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: Vec::new().into_boxed_slice(),
                }),
            },
        });

        self.bool_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Vec::new();
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
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Vec::new();
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
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Vec::new();
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
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Vec::new();
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
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Vec::new();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType {
                                nullable: true,
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
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: {
                        let mut fields = Vec::new();
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType::EQREF)),
                            mutable: true,
                        });
                        fields.push(FieldType {
                            element_type: StorageType::Val(ValType::Ref(RefType::EQREF)),
                            mutable: true,
                        });
                        fields.into_boxed_slice()
                    },
                }),
            },
        });

        self.vector_type = self.type_count;
        self.type_count += 1;
        self.types
            .ty()
            .array(&StorageType::Val(ValType::Ref(RefType::EQREF)), true);

        self.args_type = self.type_count;
        self.type_count += 1;
        self.types
            .ty()
            .array(&StorageType::Val(ValType::Ref(RefType::EQREF)), true);

        self.mut_func_ref_type = self.type_count;
        self.type_count += 1;
        self.types.ty().subtype(&SubType {
            is_final: true,
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: vec![FieldType {
                        element_type: StorageType::Val(ValType::FUNCREF),
                        mutable: true,
                    }]
                    .into_boxed_slice(),
                }),
            },
        });

        self.entrypoint_table_type = self.type_count;
        self.type_count += 1;
        self.types.ty().array(
            &StorageType::Val(ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.mut_func_ref_type),
            })),
            true,
        );

        self.closure_type = self.type_count;
        self.type_count += 1;
        self.closure_type_fields = {
            let mut fields = Vec::new();
            fields.push(FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            });
            fields.push(FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            });
            fields.push(FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.entrypoint_table_type),
                })),
                mutable: false,
            });
            fields
        };
        self.types.ty().subtype(&SubType {
            is_final: false,
            supertype_idx: None,
            composite_type: CompositeType {
                shared: false,
                inner: CompositeInnerType::Struct(StructType {
                    fields: self.closure_type_fields.clone().into_boxed_slice(),
                }),
            },
        });

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
                    nullable: true,
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
                    nullable: true,
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
                    nullable: true,
                    heap_type: HeapType::Concrete(self.bool_type),
                }),
                mutable: false,
                shared: false,
            }),
        );
        self.global_count += 1;

        for import_global in self
            .module
            .globals
            .values()
            .filter(|g| g.linkage == ir::GlobalLinkage::Import)
        {
            let global_idx = self.global_count;
            self.global_count += 1;
            let val_type = self.convert_local_type(import_global.typ);
            self.imports.import(
                "dynamic",
                &format!("global_{}", usize::from(import_global.id)),
                EntityType::Global(GlobalType {
                    val_type,
                    mutable: true,
                    shared: false,
                }),
            );
            self.global_id_to_idx.insert(import_global.id, global_idx);
        }

        for export_global in self
            .module
            .globals
            .values()
            .filter(|g| g.linkage == ir::GlobalLinkage::Export)
        {
            let global_idx = self.global_count;
            self.global_count += 1;

            let val_type = self.convert_local_type(export_global.typ);
            let init_expr = self.local_type_init_expr(export_global.typ);
            self.globals.global(
                GlobalType {
                    val_type,
                    mutable: true,
                    shared: false,
                },
                &init_expr,
            );
            self.global_id_to_idx.insert(export_global.id, global_idx);
            self.exports.export(
                &format!("global_{}", usize::from(export_global.id)),
                ExportKind::Global,
                global_idx,
            );
        }

        self.instantiate_func_func = self.add_runtime_function(
            "instantiate_func",
            WasmFuncType {
                params: vec![ValType::I32, ValType::I32, ValType::I32],
                results: vec![ValType::I32],
            },
        );
        self.instantiate_bb_func = self.add_runtime_function(
            "instantiate_bb",
            WasmFuncType {
                params: vec![
                    ValType::I32,
                    ValType::I32,
                    ValType::I32,
                    ValType::I32,
                    ValType::I32,
                ],
                results: vec![ValType::I32],
            },
        );

        self.display_func = self.add_runtime_function(
            "display",
            WasmFuncType {
                params: vec![ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.string_type),
                })],
                results: vec![],
            },
        );
        self.display_fd_func = self.add_runtime_function(
            "display_fd",
            WasmFuncType {
                params: vec![
                    ValType::I32,
                    ValType::Ref(RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(self.string_type),
                    }),
                ],
                results: vec![],
            },
        );
        self.string_to_symbol_func = self.add_runtime_function(
            "string_to_symbol",
            WasmFuncType {
                params: vec![ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.string_type),
                })],
                results: vec![ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.symbol_type),
                })],
            },
        );

        self.write_char_func = self.add_runtime_function(
            "write_char",
            WasmFuncType {
                params: vec![ValType::I32],
                results: vec![],
            },
        );
        self.int_to_string_func = self.add_runtime_function(
            "int_to_string",
            WasmFuncType {
                params: vec![ValType::I64],
                results: vec![ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.string_type),
                })],
            },
        );

        self.throw_webassembly_exception = self.add_runtime_function(
            "throw_webassembly_exception",
            WasmFuncType {
                params: vec![],
                results: vec![],
            },
        );

        for func in self.module.funcs.iter() {
            let func_idx = self.func_count;
            self.func_count += 1;

            self.func_indices.insert(func.id, func_idx);
            self.elements
                .declared(Elements::Functions(Cow::Borrowed(&[func_idx])));
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

    fn global_id_to_idx(&mut self, global: ir::GlobalId) -> u32 {
        debug_assert!(self.module.globals.contains_key(&global));

        *self.global_id_to_idx.get(&global).unwrap()
    }

    fn ref_type(&mut self, inner_ty: ir::Type) -> u32 {
        let ty = self.convert_type(inner_ty);

        *self.ref_types.entry(inner_ty).or_insert_with(|| {
            let type_id = self.type_count;
            self.type_count += 1;
            self.types.ty().struct_(vec![FieldType {
                element_type: StorageType::Val(ty),
                mutable: true,
            }]);
            type_id
        })
    }

    fn convert_local_type(&mut self, ty: ir::LocalType) -> ValType {
        match ty {
            ir::LocalType::Ref(inner_ty) => {
                let ref_type = self.ref_type(inner_ty);
                ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(ref_type),
                })
            }
            ir::LocalType::Type(ty) => self.convert_type(ty),
            ir::LocalType::VariadicArgs => ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.args_type),
            }),
            ir::LocalType::MutFuncRef => ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.mut_func_ref_type),
            }),
            ir::LocalType::EntrypointTable => ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(self.entrypoint_table_type),
            }),
            ir::LocalType::FuncRef => ValType::FUNCREF,
        }
    }

    fn convert_type(&self, ty: ir::Type) -> ValType {
        match ty {
            ir::Type::Obj => ValType::Ref(RefType::EQREF),
            ir::Type::Val(val) => match val {
                ir::ValType::Bool => ValType::I32,
                ir::ValType::Int => ValType::I64,
                ir::ValType::Char => ValType::I32,
                ir::ValType::String => ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.string_type),
                }),
                ir::ValType::Symbol => ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.symbol_type),
                }),
                ir::ValType::Nil => ValType::I32,
                ir::ValType::Cons => ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.cons_type),
                }),
                ir::ValType::Closure => ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.closure_type),
                }),
                ir::ValType::Vector => ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(self.vector_type),
                }),
            },
        }
    }

    fn local_type_init_expr(&mut self, ty: ir::LocalType) -> ConstExpr {
        match ty {
            ir::LocalType::Ref(typ) => ConstExpr::ref_null(HeapType::Concrete(self.ref_type(typ))),
            ir::LocalType::VariadicArgs => ConstExpr::ref_null(HeapType::Concrete(self.args_type)),
            ir::LocalType::MutFuncRef => {
                ConstExpr::ref_null(HeapType::Concrete(self.mut_func_ref_type))
            }
            ir::LocalType::EntrypointTable => {
                ConstExpr::ref_null(HeapType::Concrete(self.entrypoint_table_type))
            }
            ir::LocalType::FuncRef => ConstExpr::ref_null(HeapType::Abstract {
                shared: false,
                ty: AbstractHeapType::Func,
            }),
            ir::LocalType::Type(ir::Type::Obj) => ConstExpr::ref_null(HeapType::Abstract {
                shared: false,
                ty: AbstractHeapType::Eq,
            }),
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Bool)) => ConstExpr::i32_const(0),
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Int)) => ConstExpr::i64_const(0),
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Char)) => ConstExpr::i32_const(0),
            ir::LocalType::Type(ir::Type::Val(ir::ValType::String)) => {
                ConstExpr::ref_null(HeapType::Concrete(self.string_type))
            }
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Symbol)) => {
                ConstExpr::ref_null(HeapType::Concrete(self.symbol_type))
            }
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Nil)) => ConstExpr::i32_const(0),
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Cons)) => {
                ConstExpr::ref_null(HeapType::Concrete(self.cons_type))
            }
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Closure)) => {
                ConstExpr::ref_null(HeapType::Concrete(self.closure_type))
            }
            ir::LocalType::Type(ir::Type::Val(ir::ValType::Vector)) => {
                ConstExpr::ref_null(HeapType::Concrete(self.vector_type))
            }
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
        let mut is_args = VecMap::new();
        for local_id in self.func.locals.keys() {
            is_args.insert(local_id, false);
        }
        for arg in &self.func.args {
            is_args.insert(*arg, true);
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
                .values()
                .filter(|local| !is_args[local.id])
                .map(|local| {
                    let ty = self.module_generator.convert_local_type(local.typ);
                    (1, ty)
                })
                .collect::<Vec<_>>(),
        );

        let structured_bbs = reloop(self.func);
        for structured_bb in &structured_bbs {
            self.gen_bb(&mut function, self.func, structured_bb);
        }

        function.instruction(&Instruction::Unreachable); // TODO: 型チェックを通すため
        function.instruction(&Instruction::End);

        self.module_generator.functions.function(type_idx);
        self.module_generator.code.function(&function);
    }

    fn gen_bb(&mut self, function: &mut Function, func: &ir::Func, structured_bb: &Structured) {
        match structured_bb {
            Structured::Simple(bb_id) => {
                let bb = &func.bbs[*bb_id];
                for expr in &bb.exprs {
                    self.gen_assign(function, &func.locals, expr);
                }
            }
            Structured::If { cond, then, else_ } => {
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
            Structured::Block { body } => {
                function.instruction(&Instruction::Block(BlockType::Empty));
                for structured_bb in body {
                    self.gen_bb(function, func, structured_bb);
                }
                function.instruction(&Instruction::End);
            }
            Structured::Loop { body } => {
                function.instruction(&Instruction::Loop(BlockType::Empty));
                for structured_bb in body {
                    self.gen_bb(function, func, structured_bb);
                }
                function.instruction(&Instruction::End);
            }
            Structured::Break(index) => {
                function.instruction(&Instruction::Br(*index as u32));
            }
            Structured::Terminator(terminator) => match terminator {
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
                BasicBlockTerminator::TailCallClosure(..) => {
                    unreachable!("unexpected TailCallClosure");
                }
                BasicBlockTerminator::Error(msg) => {
                    function.instruction(&Instruction::I32Const(2));
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*msg)));
                    function.instruction(&Instruction::Call(self.module_generator.display_fd_func));
                    function.instruction(&Instruction::Call(
                        self.module_generator.throw_webassembly_exception,
                    ));
                }
            },
        }
    }

    fn gen_assign(
        &mut self,
        function: &mut Function,
        locals: &VecMap<ir::LocalId, ir::Local>,
        expr: &ir::ExprAssign,
    ) {
        if let ir::Expr::Nop = expr.expr {
            // desugarである程度は削除しているが、その後の最適化で再度Nopが発生することがあるためここでも除去
            debug_assert!(expr.local.is_none());
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
        locals: &VecMap<ir::LocalId, ir::Local>,
        expr: &ir::Expr,
    ) {
        match expr {
            ir::Expr::Nop => {
                unreachable!("unexpected Nop");
            }
            ir::Expr::Phi(..) => {
                unreachable!("unexpected Phi");
            }
            ir::Expr::CallClosure(..) => {
                unreachable!("unexpected CallClosure");
            }
            ir::Expr::InstantiateFunc(module_id, func_id, func_index) => {
                function.instruction(&Instruction::I32Const(usize::from(*module_id) as i32));
                function.instruction(&Instruction::I32Const(usize::from(*func_id) as i32));
                function.instruction(&Instruction::I32Const(*func_index as i32));
                function.instruction(&Instruction::Call(
                    self.module_generator.instantiate_func_func,
                ));
            }
            ir::Expr::InstantiateClosureFunc(module_id, func_id, func_index) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*module_id)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*func_id)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::I32Const(*func_index as i32));
                function.instruction(&Instruction::Call(
                    self.module_generator.instantiate_func_func,
                ));
            }
            ir::Expr::InstantiateBB(module_id, func_id, func_index, bb_id, index) => {
                function.instruction(&Instruction::I32Const(usize::from(*module_id) as i32));
                function.instruction(&Instruction::I32Const(usize::from(*func_id) as i32));
                function.instruction(&Instruction::I32Const(*func_index as i32));
                function.instruction(&Instruction::I32Const(usize::from(*bb_id) as i32));
                function.instruction(&Instruction::I32Const(*index as i32));
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
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*car)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*cdr)));
                function.instruction(&Instruction::StructNew(self.module_generator.cons_type));
            }
            ir::Expr::Vector(vec) => {
                for elem in vec.iter() {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*elem)));
                }
                function.instruction(&Instruction::ArrayNewFixed {
                    array_type_index: self.module_generator.vector_type,
                    array_size: vec.len() as u32,
                });
            }
            ir::Expr::CreateRef(typ) => {
                function.instruction(&Instruction::RefNull(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::Eq,
                }));
                function.instruction(&Instruction::StructNew(
                    self.module_generator.ref_type(*typ),
                ));
            }
            ir::Expr::DerefRef(typ, ref_) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*ref_)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.ref_type(*typ),
                    field_index: ModuleGenerator::REF_VALUE_FIELD,
                });
            }
            ir::Expr::SetRef(typ, ref_, val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*ref_)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::StructSet {
                    struct_type_index: self.module_generator.ref_type(*typ),
                    field_index: ModuleGenerator::REF_VALUE_FIELD,
                });
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
            }
            ir::Expr::FuncRef(func) => {
                let func_idx = self.module_generator.func_indices[func];
                function.instruction(&Instruction::RefFunc(func_idx));
            }
            ir::Expr::Closure {
                envs,
                module_id,
                func_id,
                entrypoint_table,
            } => {
                function.instruction(&Instruction::I32Const(usize::from(*module_id) as i32));
                function.instruction(&Instruction::I32Const(usize::from(*func_id) as i32));
                function.instruction(&Instruction::LocalGet(
                    self.local_id_to_idx(*entrypoint_table),
                ));
                for env in envs.iter() {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*env)));
                }

                function.instruction(&Instruction::StructNew(
                    self.module_generator.closure_type_from_ir(
                        &envs.iter().map(|env| locals[*env].typ).collect::<Vec<_>>(),
                    ),
                ));
            }
            ir::Expr::CallRef(call_ref) => {
                self.gen_call_ref(function, false, call_ref);
            }
            ir::Expr::ClosureModuleId(closure) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*closure)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.closure_type,
                    field_index: ModuleGenerator::CLOSURE_MODULE_ID_FIELD,
                });
                function.instruction(&Instruction::I64ExtendI32S);
            }
            ir::Expr::ClosureFuncId(closure) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*closure)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.closure_type,
                    field_index: ModuleGenerator::CLOSURE_FUNC_ID_FIELD,
                });
                function.instruction(&Instruction::I64ExtendI32S);
            }
            ir::Expr::ClosureEntrypointTable(closure) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*closure)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.closure_type,
                    field_index: ModuleGenerator::CLOSURE_ENTRYPOINT_TABLE_FIELD,
                });
            }
            ir::Expr::Call(call) => {
                self.gen_call(function, false, call);
            }
            ir::Expr::Move(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
            }
            ir::Expr::FromObj(typ, val) => match typ {
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
            },
            ir::Expr::ToObj(typ, val) => match typ {
                ir::ValType::Bool => {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::If(BlockType::Result(ValType::Ref(
                        RefType {
                            nullable: true,
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
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                    function.instruction(&Instruction::StructNew(self.module_generator.int_type));
                }
                ir::ValType::Char => {
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
                function.instruction(&Instruction::GlobalGet(
                    self.module_generator.global_id_to_idx(*global),
                ));
            }
            ir::Expr::GlobalSet(global, val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::GlobalSet(
                    self.module_generator.global_id_to_idx(*global),
                ));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
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
            ir::Expr::Is(typ, val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    match typ {
                        ir::ValType::Bool => self.module_generator.bool_type,
                        ir::ValType::Int => self.module_generator.int_type,
                        ir::ValType::Char => self.module_generator.char_type,
                        ir::ValType::String => self.module_generator.string_type,
                        ir::ValType::Symbol => self.module_generator.symbol_type,
                        ir::ValType::Nil => self.module_generator.nil_type,
                        ir::ValType::Cons => self.module_generator.cons_type,
                        ir::ValType::Closure => self.module_generator.closure_type,
                        ir::ValType::Vector => self.module_generator.vector_type,
                    },
                )));
            }
            ir::Expr::VectorLength(val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::ArrayLen);
                function.instruction(&Instruction::I64ExtendI32U);
            }
            ir::Expr::VectorRef(vector, index) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*vector)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*index)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::ArrayGet(self.module_generator.vector_type));
            }
            ir::Expr::VectorSet(vector, index, val) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*vector)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*index)));
                function.instruction(&Instruction::I32WrapI64);
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*val)));
                function.instruction(&Instruction::ArraySet(self.module_generator.vector_type));
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
            ir::Expr::VariadicArgs(args) => {
                for arg in args.iter() {
                    function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*arg)));
                }
                function.instruction(&Instruction::ArrayNewFixed {
                    array_type_index: self.module_generator.args_type,
                    array_size: args.len() as u32,
                });
            }
            ir::Expr::VariadicArgsRef(args, idx) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*args)));
                function.instruction(&Instruction::I32Const(*idx as i32));
                function.instruction(&Instruction::ArrayGet(self.module_generator.args_type));
            }
            ir::Expr::VariadicArgsLength(args) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*args)));
                function.instruction(&Instruction::ArrayLen);
                function.instruction(&Instruction::I64ExtendI32U);
            }
            ir::Expr::CreateMutFuncRef(id) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*id)));
                function.instruction(&Instruction::StructNew(
                    self.module_generator.mut_func_ref_type,
                ));
            }
            ir::Expr::CreateEmptyMutFuncRef => {
                function.instruction(&Instruction::RefNull(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::Func,
                }));
                function.instruction(&Instruction::StructNew(
                    self.module_generator.mut_func_ref_type,
                ));
            }
            ir::Expr::DerefMutFuncRef(mut_func_ref) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*mut_func_ref)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.module_generator.mut_func_ref_type,
                    field_index: ModuleGenerator::MUT_FUNC_REF_FUNC_FIELD,
                });
            }
            ir::Expr::SetMutFuncRef(mut_func_ref, func_ref) => {
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*mut_func_ref)));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*func_ref)));
                function.instruction(&Instruction::StructSet {
                    struct_type_index: self.module_generator.mut_func_ref_type,
                    field_index: ModuleGenerator::MUT_FUNC_REF_FUNC_FIELD,
                });
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::EntrypointTable(mut_func_refs) => {
                for mut_func_ref in mut_func_refs.iter() {
                    function
                        .instruction(&Instruction::LocalGet(self.local_id_to_idx(*mut_func_ref)));
                }
                function.instruction(&Instruction::ArrayNewFixed {
                    array_type_index: self.module_generator.entrypoint_table_type,
                    array_size: mut_func_refs.len() as u32,
                });
            }
            ir::Expr::EntrypointTableRef(index, entrypoint_table) => {
                function.instruction(&Instruction::LocalGet(
                    self.local_id_to_idx(*entrypoint_table),
                ));
                function.instruction(&Instruction::I32Const(*index as i32));
                function.instruction(&Instruction::ArrayGet(
                    self.module_generator.entrypoint_table_type,
                ));
            }
            ir::Expr::SetEntrypointTable(index, entrypoint_table, mut_func_ref) => {
                function.instruction(&Instruction::LocalGet(
                    self.local_id_to_idx(*entrypoint_table),
                ));
                function.instruction(&Instruction::I32Const(*index as i32));
                function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*mut_func_ref)));
                function.instruction(&Instruction::ArraySet(
                    self.module_generator.entrypoint_table_type,
                ));
                function.instruction(&Instruction::I32Const(0));
            }
        }
    }

    fn gen_call_ref(&mut self, function: &mut Function, is_tail: bool, call_ref: &ir::ExprCallRef) {
        let func_type = self.module_generator.func_type_from_ir(&call_ref.func_type);

        for arg in &call_ref.args {
            function.instruction(&Instruction::LocalGet(self.local_id_to_idx(*arg)));
        }

        function.instruction(&Instruction::LocalGet(self.local_id_to_idx(call_ref.func)));
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
