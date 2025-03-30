use rustc_hash::FxHashMap;
use std::borrow::Cow;
use typed_index_collections::{TiSlice, ti_vec};

use crate::ir::BasicBlockNext;

use super::ir;
use wasm_encoder::{
    AbstractHeapType, BlockType, CodeSection, CompositeInnerType, CompositeType, DataCountSection,
    DataSection, ElementSection, Elements, EntityType, ExportKind, ExportSection, FieldType,
    FuncType, Function, FunctionSection, GlobalSection, GlobalType, HeapType, ImportSection,
    Instruction, MemoryType, Module, RefType, StorageType, StructType, SubType, TableSection,
    TableType, TypeSection, ValType,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WasmFuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

#[derive(Debug, Clone, Copy)]

struct FuncIndex {
    func_idx: u32,
    boxed_func_idx: u32,
}

#[derive(Debug)]
pub struct WasmGenerator {}

impl Default for WasmGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmGenerator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn generate(&mut self, ir: &ir::Ir) -> Vec<u8> {
        let mut module_gen = ModuleGenerator::new();
        let module = module_gen.generate(ir);
        module.finish()
    }
}

#[derive(Debug)]
struct ModuleGenerator {
    type_count: u32,
    func_count: u32,
    global_count: u32,
    table_count: u32,
    // runtime functions
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
    func_indices: FxHashMap<ir::FuncId, FuncIndex>,
    exports: ExportSection,
    // types
    mut_cell_type: u32,
    nil_type: u32,
    bool_type: u32,
    int_type: u32,
    char_type: u32,
    cons_type: u32,
    buf_type: u32,
    string_buf_type: u32,
    string_type: u32,
    symbol_type: u32,
    variable_params_type: u32,
    boxed_func_type: u32,
    closure_type: u32,
    closure_types: FxHashMap<Vec<ValType>, u32>, // env types -> type index
    func_types: FxHashMap<WasmFuncType, u32>,
    closure_type_fields: Vec<FieldType>,
    // table
    global_table: u32,
    // const
    nil_global: Option<u32>,
    true_global: Option<u32>,
    false_global: Option<u32>,
}

impl ModuleGenerator {
    fn new() -> Self {
        Self {
            type_count: 0,
            func_count: 0,
            global_count: 0,
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
            mut_cell_type: 0,
            nil_type: 0,
            bool_type: 0,
            int_type: 0,
            char_type: 0,
            cons_type: 0,
            buf_type: 0,
            string_buf_type: 0,
            string_type: 0,
            symbol_type: 0,
            variable_params_type: 0,
            boxed_func_type: 0,
            closure_type: 0,
            closure_types: FxHashMap::default(),
            func_types: FxHashMap::default(),
            closure_type_fields: Vec::new(),
            global_table: 0,
            nil_global: None,
            true_global: None,
            false_global: None,
            table_count: 0,
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
        if let Some(type_index) = self.func_types.get(&func_type) {
            *type_index
        } else {
            let type_index = self.type_count;
            self.type_count += 1;
            self.types
                .ty()
                .function(func_type.params.clone(), func_type.results.clone());
            self.func_types.insert(func_type, type_index);
            type_index
        }
    }

    fn func_type_from_ir(&mut self, ir_func_type: ir::FuncType) -> u32 {
        self.func_type(WasmFuncType {
            params: ir_func_type
                .args
                .into_iter()
                .map(|ty| self.convert_type(ty))
                .collect(),
            results: ir_func_type
                .rets
                .into_iter()
                .map(|ty| self.convert_type(ty))
                .collect(),
        })
    }

    fn closure_type(&mut self, env_types: Vec<ValType>) -> u32 {
        if let Some(type_index) = self.closure_types.get(&env_types) {
            *type_index
        } else {
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

            self.closure_types.insert(env_types, type_index);

            type_index
        }
    }

    fn closure_type_from_ir(&mut self, env_types: Vec<ir::LocalType>) -> u32 {
        self.closure_type(
            env_types
                .into_iter()
                .map(|ty| self.convert_local_type(ty))
                .collect(),
        )
    }

    const BOXED_TYPE: ValType = ValType::Ref(RefType::EQREF);
    const MUT_CELL_VALUE_FIELD: u32 = 0;
    const BOOL_VALUE_FIELD: u32 = 0;
    const INT_VALUE_FIELD: u32 = 0;
    const CHAR_VALUE_FIELD: u32 = 0;
    const CONS_CAR_FIELD: u32 = 0;
    const CONS_CDR_FIELD: u32 = 1;
    const SYMBOL_STRING_FIELD: u32 = 0;
    const CLOSURE_FUNC_FIELD: u32 = 0;
    // const CLOSURE_BOXED_FUNC_FIELD: u32 = 1;
    const CLOSURE_ENVS_FIELD_OFFSET: u32 = 2;
    // const STRING_BUF_BUF_FIELD: u32 = 0;
    // const STRING_BUF_SHARED_FIELD: u32 = 1;
    // const STRING_BUF_FIELD: u32 = 0;
    // const STRING_LEN_FIELD: u32 = 1;
    // const STRING_OFFSET_FIELD: u32 = 2;

    pub fn generate(&mut self, ir: &ir::Ir) -> Module {
        self.mut_cell_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![FieldType {
            element_type: StorageType::Val(Self::BOXED_TYPE),
            mutable: true,
        }]);

        self.nil_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![]);

        self.bool_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![FieldType {
            element_type: StorageType::I8,
            mutable: false,
        }]);

        self.int_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![FieldType {
            element_type: StorageType::Val(ValType::I64),
            mutable: false,
        }]);

        self.char_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![FieldType {
            element_type: StorageType::Val(ValType::I32),
            mutable: false,
        }]);

        self.cons_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(Self::BOXED_TYPE),
                mutable: true,
            },
            FieldType {
                element_type: StorageType::Val(Self::BOXED_TYPE),
                mutable: true,
            },
        ]);

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

        self.string_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.string_buf_type),
                })),
                mutable: true,
            },
            FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            },
            FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            },
        ]);

        self.symbol_type = self.type_count;
        self.type_count += 1;
        self.types.ty().struct_(vec![FieldType {
            element_type: StorageType::Val(ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.string_type),
            })),
            mutable: false,
        }]);

        self.variable_params_type = self.type_count;
        self.type_count += 1;
        self.types
            .ty()
            .array(&StorageType::Val(Self::BOXED_TYPE), false);

        self.boxed_func_type = self.type_count;
        self.type_count += 1;
        self.closure_type = self.type_count;
        self.type_count += 1;
        self.closure_type_fields = vec![
            FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Func,
                    },
                })),
                mutable: false,
            },
            FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(self.boxed_func_type),
                })),
                mutable: false,
            },
        ];
        self.types.ty().rec(vec![
            SubType {
                is_final: true,
                supertype_idx: None,
                composite_type: CompositeType {
                    shared: false,
                    inner: CompositeInnerType::Func(FuncType::new(
                        [
                            ValType::Ref(RefType {
                                nullable: false,
                                heap_type: HeapType::Concrete(self.closure_type),
                            }),
                            ValType::Ref(RefType {
                                nullable: false,
                                heap_type: HeapType::Concrete(self.variable_params_type),
                            }),
                        ],
                        [Self::BOXED_TYPE],
                    )),
                },
            },
            SubType {
                is_final: false,
                supertype_idx: None,
                composite_type: CompositeType {
                    shared: false,
                    inner: CompositeInnerType::Struct(StructType {
                        fields: self.closure_type_fields.clone().into_boxed_slice(),
                    }),
                },
            },
        ]);

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
                element_type: RefType::EQREF,
                table64: false,
                minimum: 0,
                maximum: None,
                shared: false,
            }),
        );

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

        for func in ir.funcs.iter() {
            let type_idx = self.func_type_from_ir(func.func_type());

            let func_idx = self.func_count;
            self.func_count += 1;

            let boxed_func_idx = self.func_count;
            self.func_count += 1;

            let mut function = Function::new(
                func.locals
                    .iter()
                    .skip(func.args)
                    .map(|ty| {
                        let ty = self.convert_local_type(*ty);
                        (1, ty)
                    })
                    .collect::<Vec<_>>(),
            );

            // TODO: きちんとrelooperを実装する
            // 現在の問題点: if (x) { a } else { b } cと生成するべきところを if (x) { a; c } else { b; c }とcを重複して生成してしまうので非効率的End,
            // TODO: ModuleGeneratorのメソッドにする
            fn gen_bb(
                genetator: &mut ModuleGenerator,
                func: &ir::Func,
                function: &mut Function,
                bb_id: ir::BasicBlockId,
            ) {
                let bb = &func.bbs[bb_id];
                for expr in &bb.exprs {
                    genetator.gen_assign(function, &func.locals, expr);
                }

                match bb.next {
                    BasicBlockNext::Jump(target) => {
                        gen_bb(genetator, func, function, target);
                    }
                    BasicBlockNext::If(cond, then_target, else_target) => {
                        function.instruction(&Instruction::LocalGet(
                            ModuleGenerator::from_local_id(cond),
                        ));
                        function.instruction(&Instruction::If(BlockType::Empty));
                        gen_bb(genetator, func, function, then_target);
                        function.instruction(&Instruction::Else);
                        gen_bb(genetator, func, function, else_target);
                        function.instruction(&Instruction::End);
                    }
                    BasicBlockNext::Return => {
                        for ret in &func.rets {
                            function.instruction(&Instruction::LocalGet(
                                ModuleGenerator::from_local_id(*ret),
                            ));
                        }
                        function.instruction(&Instruction::Return);
                    }
                }
            }
            gen_bb(self, func, &mut function, func.bb_entry);

            function.instruction(&Instruction::Unreachable); // TODO: 型チェックを通すため
            function.instruction(&Instruction::End);

            self.func_indices.insert(func.id, FuncIndex {
                func_idx,
                boxed_func_idx,
            });
            self.elements.declared(Elements::Functions(Cow::Borrowed(&[
                func_idx,
                boxed_func_idx,
            ])));

            self.functions.function(type_idx);
            self.code.function(&function);

            // TODO: boxed_func
            self.functions.function(self.boxed_func_type);
            self.code.function(&{
                let mut function = Function::new(Vec::new());
                function.instruction(&Instruction::Unreachable);
                function.instruction(&Instruction::End);
                function
            });
        }

        self.exports.export(
            "start",
            ExportKind::Func,
            self.func_indices[&ir.entry].func_idx,
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

    // TODO: きちんとHashMapなどで管理
    fn from_local_id(local: ir::LocalId) -> u32 {
        usize::from(local) as u32
    }

    fn from_global_id(global: ir::GlobalId) -> i32 {
        usize::from(global) as i32
    }

    fn gen_assign(
        &mut self,
        function: &mut Function,
        locals: &TiSlice<ir::LocalId, ir::LocalType>,
        expr: &ir::ExprAssign,
    ) {
        self.gen_expr(function, locals, &expr.expr);
        if let Some(local) = &expr.local {
            function.instruction(&Instruction::LocalSet(Self::from_local_id(*local)));
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
                let data_index = self.datas.len();
                self.datas.passive(bs.iter().copied());

                // data offset
                function.instruction(&Instruction::I32Const(0));
                // data len
                function.instruction(&Instruction::I32Const(bs.len() as i32));
                // StringBuf.buf
                function.instruction(&Instruction::ArrayNewData {
                    array_type_index: self.buf_type,
                    array_data_index: data_index,
                });

                // StringBuf.shared
                function.instruction(&Instruction::I32Const(0));
                // String.buf
                function.instruction(&Instruction::StructNew(self.string_buf_type));

                // String.len
                function.instruction(&Instruction::I32Const(bs.len() as i32));
                // String.offset
                function.instruction(&Instruction::I32Const(0));
                function.instruction(&Instruction::StructNew(self.string_type));
            }
            ir::Expr::StringToSymbol(s) => {
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*s)));
                function.instruction(&Instruction::Call(self.string_to_symbol_func));
            }
            ir::Expr::Nil => {
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::Cons(car, cdr) => {
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*car)));
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*cdr)));
                function.instruction(&Instruction::StructNew(self.cons_type));
            }
            ir::Expr::CreateMutCell => {
                function.instruction(&Instruction::RefNull(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::Eq,
                }));
                function.instruction(&Instruction::StructNew(self.mut_cell_type));
            }
            ir::Expr::DerefMutCell(cell) => {
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*cell)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.mut_cell_type,
                    field_index: Self::MUT_CELL_VALUE_FIELD,
                });
            }
            ir::Expr::SetMutCell(cell, val) => {
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*cell)));
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                function.instruction(&Instruction::StructSet {
                    struct_type_index: self.mut_cell_type,
                    field_index: Self::MUT_CELL_VALUE_FIELD,
                });
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
            }
            ir::Expr::Closure(envs, func) => {
                let func_idx = self.func_indices[func];

                function.instruction(&Instruction::RefFunc(func_idx.func_idx));
                function.instruction(&Instruction::RefFunc(func_idx.boxed_func_idx));
                for env in envs.iter() {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*env)));
                }

                function.instruction(&Instruction::StructNew(
                    self.closure_type_from_ir(envs.iter().map(|env| locals[*env]).collect()),
                ));
            }
            ir::Expr::CallClosure(tail_call, closure, args) => {
                let func_type = self.func_type_from_ir(ir::FuncType {
                    args: args.iter().map(|arg| locals[*arg].to_type()).collect(),
                    rets: ti_vec![ir::Type::Boxed],
                });

                for arg in args {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*arg)));
                }

                function.instruction(&Instruction::LocalGet(Self::from_local_id(*closure)));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.closure_type,
                    field_index: Self::CLOSURE_FUNC_FIELD,
                });
                function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(func_type)));
                if *tail_call {
                    function.instruction(&Instruction::ReturnCallRef(func_type));
                } else {
                    function.instruction(&Instruction::CallRef(func_type));
                }
            }
            ir::Expr::Move(val) => {
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
            }
            ir::Expr::Unbox(typ, val) => match typ {
                ir::ValType::Bool => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.bool_type,
                    )));
                    function.instruction(&Instruction::StructGetU {
                        struct_type_index: self.bool_type,
                        field_index: Self::BOOL_VALUE_FIELD,
                    });
                }
                ir::ValType::Int => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.int_type,
                    )));
                    function.instruction(&Instruction::StructGet {
                        struct_type_index: self.int_type,
                        field_index: Self::INT_VALUE_FIELD,
                    });
                }
                ir::ValType::Char => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.char_type,
                    )));
                    function.instruction(&Instruction::StructGet {
                        struct_type_index: self.char_type,
                        field_index: Self::CHAR_VALUE_FIELD,
                    });
                }
                ir::ValType::String => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.string_type,
                    )));
                }
                ir::ValType::Symbol => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.symbol_type,
                    )));
                }
                ir::ValType::Nil => {
                    // TODO: 型チェックするべき
                    function.instruction(&Instruction::I32Const(0));
                }
                ir::ValType::Cons => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.cons_type,
                    )));
                }
                ir::ValType::Closure => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                        self.closure_type,
                    )));
                }
            },
            ir::Expr::Box(typ, val) => match typ {
                ir::ValType::Bool => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::If(BlockType::Result(ValType::Ref(
                        RefType {
                            nullable: false,
                            heap_type: HeapType::Concrete(self.bool_type),
                        },
                    ))));
                    function.instruction(&Instruction::GlobalGet(self.true_global.unwrap()));
                    function.instruction(&Instruction::Else);
                    function.instruction(&Instruction::GlobalGet(self.false_global.unwrap()));
                    function.instruction(&Instruction::End);
                }
                ir::ValType::Int => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::StructNew(self.int_type));
                }
                ir::ValType::Char => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                    function.instruction(&Instruction::StructNew(self.char_type));
                }
                ir::ValType::String => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                }
                ir::ValType::Symbol => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                }
                ir::ValType::Nil => {
                    function.instruction(&Instruction::GlobalGet(self.nil_global.unwrap()));
                }
                ir::ValType::Cons => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                }
                ir::ValType::Closure => {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                }
            },
            ir::Expr::ClosureEnv(env_types, closure, env_index) => {
                let closure_type = self.closure_type_from_ir(env_types.clone());
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*closure)));
                // TODO: irでキャストするべき
                function.instruction(&Instruction::RefCastNonNull(HeapType::Concrete(
                    closure_type,
                )));
                function.instruction(&Instruction::StructGet {
                    struct_type_index: closure_type,
                    field_index: Self::CLOSURE_ENVS_FIELD_OFFSET + *env_index as u32,
                });
            }
            ir::Expr::GlobalGet(global) => {
                function.instruction(&Instruction::I32Const(Self::from_global_id(*global)));
                function.instruction(&Instruction::TableGet(self.global_table));
            }
            ir::Expr::GlobalSet(global, val) => {
                function.instruction(&Instruction::I32Const(Self::from_global_id(*global)));
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
                function.instruction(&Instruction::TableSet(self.global_table));
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*val)));
            }
            ir::Expr::Error(msg) => {
                function.instruction(&Instruction::I32Const(2));
                function.instruction(&Instruction::LocalGet(Self::from_local_id(*msg)));
                function.instruction(&Instruction::Call(self.display_fd_func));
                function.instruction(&Instruction::Call(self.throw_webassembly_exception));
                // これがないとこの後のdropでコンパイルエラーになる
                function.instruction(&Instruction::RefNull(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::Eq,
                }));
            }
            ir::Expr::Builtin(builtin, args) => {
                for arg in args {
                    function.instruction(&Instruction::LocalGet(Self::from_local_id(*arg)));
                }
                self.gen_builtin(*builtin, function);
            }
            ir::Expr::InitGlobals(n) => {
                // 必要なサイズになるまで2倍に拡張
                function.instruction(&Instruction::Block(BlockType::Empty));
                function.instruction(&Instruction::Loop(BlockType::Empty));
                function.instruction(&Instruction::TableSize(self.global_table));
                function.instruction(&Instruction::I32Const(*n as i32));
                function.instruction(&Instruction::I32GeU);
                function.instruction(&Instruction::BrIf(1));
                function.instruction(&Instruction::RefNull(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::Eq,
                }));
                function.instruction(&Instruction::TableSize(self.global_table));
                function.instruction(&Instruction::TableGrow(self.global_table));
                function.instruction(&Instruction::I32Const(-1));
                function.instruction(&Instruction::I32Eq);
                function.instruction(&Instruction::If(BlockType::Empty));
                function.instruction(&Instruction::Unreachable);
                function.instruction(&Instruction::End);
                function.instruction(&Instruction::Br(0));
                function.instruction(&Instruction::End);
                function.instruction(&Instruction::End);

                function.instruction(&Instruction::GlobalGet(self.nil_global.unwrap()));
            }
        }
    }

    fn convert_local_type(&self, ty: ir::LocalType) -> ValType {
        match ty {
            ir::LocalType::MutCell => ValType::Ref(RefType {
                nullable: false,
                heap_type: HeapType::Concrete(self.mut_cell_type),
            }),
            ir::LocalType::Type(ty) => self.convert_type(ty),
        }
    }

    fn convert_type(&self, ty: ir::Type) -> ValType {
        match ty {
            ir::Type::Boxed => Self::BOXED_TYPE,
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
            },
        }
    }

    fn gen_builtin(&self, builtin: ir::Builtin, function: &mut Function) {
        match builtin {
            ir::Builtin::Display => {
                function.instruction(&Instruction::Call(self.display_func));
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Builtin::Add => {
                function.instruction(&Instruction::I64Add);
            }
            ir::Builtin::Sub => {
                function.instruction(&Instruction::I64Sub);
            }
            ir::Builtin::WriteChar => {
                function.instruction(&Instruction::Call(self.write_char_func));
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Builtin::IsPair => {
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.cons_type,
                )));
            }
            ir::Builtin::IsSymbol => {
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.symbol_type,
                )));
            }
            ir::Builtin::IsString => {
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.string_type,
                )));
            }
            ir::Builtin::IsChar => {
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.char_type,
                )));
            }
            ir::Builtin::IsNumber => {
                // TODO: 一般のnumberかを判定
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.int_type,
                )));
            }
            ir::Builtin::IsBoolean => {
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.bool_type,
                )));
            }
            ir::Builtin::IsProcedure => {
                function.instruction(&Instruction::RefTestNonNull(HeapType::Concrete(
                    self.closure_type,
                )));
            }
            ir::Builtin::Eq => {
                function.instruction(&Instruction::RefEq);
            }
            ir::Builtin::Car => {
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.cons_type,
                    field_index: Self::CONS_CAR_FIELD,
                });
            }
            ir::Builtin::Cdr => {
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.cons_type,
                    field_index: Self::CONS_CDR_FIELD,
                });
            }
            ir::Builtin::SymbolToString => {
                function.instruction(&Instruction::StructGet {
                    struct_type_index: self.symbol_type,
                    field_index: Self::SYMBOL_STRING_FIELD,
                });
            }
            ir::Builtin::NumberToString => {
                // TODO: 一般のnumberに対応
                function.instruction(&Instruction::Call(self.int_to_string_func));
            }
            ir::Builtin::EqNum => {
                function.instruction(&Instruction::I64Eq);
            }
            ir::Builtin::Lt => {
                function.instruction(&Instruction::I64LtS);
            }
            ir::Builtin::Gt => {
                function.instruction(&Instruction::I64GtS);
            }
            ir::Builtin::Le => {
                function.instruction(&Instruction::I64LeS);
            }
            ir::Builtin::Ge => {
                function.instruction(&Instruction::I64GeS);
            }
        }
    }
}
