use std::collections::HashMap;

use crate::ast;

use super::ir;
use std::borrow::Cow;
use strum::IntoEnumIterator;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, ElementSection, Elements, EntityType, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction, MemArg, MemoryType,
    Module, RefType, StartSection, TableSection, TableType, TypeSection, ValType,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WasmFuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

impl WasmFuncType {
    pub fn from_ir(ir_func_type: ir::FuncType) -> Self {
        Self {
            params: ir_func_type
                .args
                .into_iter()
                .map(ModuleGenerator::convert_type)
                .collect(),
            results: ir_func_type
                .rets
                .into_iter()
                .map(ModuleGenerator::convert_type)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy)]

struct FuncIndex {
    func_idx: u32,
    elem_idx: u32,
}

#[derive(Debug)]
pub struct Codegen {
    element_offset: usize,
}

impl Codegen {
    pub fn new() -> Self {
        Self { element_offset: 0 }
    }

    pub fn gen(&mut self, ir: &ir::Ir) -> anyhow::Result<Vec<u8>> {
        let mut module_gen = ModuleGenerator::new(self.element_offset);
        let module = module_gen.gen(&ir);
        self.element_offset += module_gen.element_funcs.len();
        Ok(module.finish())
    }
}

#[derive(Debug)]
struct ModuleGenerator {
    func_to_type_index: HashMap<WasmFuncType, u32>,
    type_count: u32,
    func_count: u32,
    global_count: u32,
    // runtime functions
    malloc_func: u32,
    dump_func: u32,
    string_to_symbol_func: u32,
    get_global_func: u32,
    get_builtin_func: u32,
    // wasm section
    imports: ImportSection,
    types: TypeSection,
    functions: FunctionSection,
    tables: TableSection,
    code: CodeSection,
    globals: GlobalSection,
    malloc_tmp_global: u32,
    element_funcs: Vec<u32>,
    func_indices: HashMap<usize, FuncIndex>,
    global_to_index: HashMap<usize, u32>,
    builtin_to_global: HashMap<ast::Builtin, u32>,
    element_offset: usize,
}

impl ModuleGenerator {
    fn new(element_offset: usize) -> Self {
        Self {
            func_to_type_index: HashMap::new(),
            type_count: 0,
            func_count: 0,
            global_count: 0,
            malloc_func: 0,
            dump_func: 0,
            string_to_symbol_func: 0,
            get_global_func: 0,
            get_builtin_func: 0,
            malloc_tmp_global: 0,
            imports: ImportSection::new(),
            types: TypeSection::new(),
            functions: FunctionSection::new(),
            tables: TableSection::new(),
            code: CodeSection::new(),
            globals: GlobalSection::new(),
            element_funcs: Vec::new(),
            func_indices: HashMap::new(),
            global_to_index: HashMap::new(),
            builtin_to_global: HashMap::new(),
            element_offset,
        }
    }

    fn add_runtime_function(&mut self, name: &str, func_type: WasmFuncType) -> u32 {
        let type_index = self.func_type(&func_type);
        self.imports
            .import("runtime", name, EntityType::Function(type_index));
        let func_index = self.func_count;
        self.func_count += 1;
        func_index
    }

    fn func_type(&mut self, func_type: &WasmFuncType) -> u32 {
        if let Some(type_index) = self.func_to_type_index.get(&func_type) {
            *type_index
        } else {
            self.types
                .ty()
                .function(func_type.params.clone(), func_type.results.clone());
            let type_index = self.type_count;
            self.func_to_type_index
                .insert(func_type.clone(), type_index);
            self.type_count += 1;
            type_index
        }
    }

    pub fn gen(&mut self, ir: &ir::Ir) -> Module {
        self.imports.import(
            "runtime",
            "memory",
            MemoryType {
                minimum: 1,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            },
        );

        self.imports.import(
            "runtime",
            "funcs",
            TableType {
                element_type: RefType::FUNCREF,
                minimum: 1,
                maximum: None,
                table64: false,
                shared: false,
            },
        );

        self.malloc_func = self.add_runtime_function(
            "malloc",
            WasmFuncType {
                params: vec![ValType::I32],
                results: vec![ValType::I32],
            },
        );
        self.dump_func = self.add_runtime_function(
            "dump",
            WasmFuncType {
                params: vec![ValType::I64],
                results: vec![],
            },
        );
        self.string_to_symbol_func = self.add_runtime_function(
            "string_to_symbol",
            WasmFuncType {
                params: vec![ValType::I32],
                results: vec![ValType::I32],
            },
        );
        self.get_global_func = self.add_runtime_function(
            "get_global",
            WasmFuncType {
                params: vec![ValType::I32],
                results: vec![ValType::I32],
            },
        );
        self.get_builtin_func = self.add_runtime_function(
            "get_builtin",
            WasmFuncType {
                params: vec![ValType::I32],
                results: vec![ValType::I32],
            },
        );

        self.malloc_tmp_global = self.global_count;
        self.globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(0),
        );
        self.global_count += 1;

        for global in 0..ir.global_count {
            let global_index = self.global_count;
            self.globals.global(
                GlobalType {
                    val_type: ValType::I32,
                    mutable: true,
                    shared: false,
                },
                // TODO: nilか0で初期化
                &ConstExpr::i32_const(0),
            );
            self.global_count += 1;
            self.global_to_index.insert(global, global_index);
        }

        for builtin in ast::Builtin::iter() {
            let global_index = self.global_count;
            self.globals.global(
                GlobalType {
                    val_type: ValType::I32,
                    mutable: true,
                    shared: false,
                },
                &ConstExpr::i32_const(0),
            );
            self.global_count += 1;
            self.builtin_to_global.insert(builtin, global_index);
        }

        for (i, func) in ir.funcs.iter().enumerate() {
            let type_index = self.func_type(&WasmFuncType::from_ir(func.func_type()));

            let mut function = Function::new(
                func.locals
                    .iter()
                    .skip(func.args)
                    .map(|ty| {
                        let ty = Self::convert_type(*ty);
                        (1, ty)
                    })
                    .collect::<Vec<_>>(),
            );

            // body
            for stmt in &func.body {
                self.gen_stat(&mut function, &func.locals, stmt);
            }

            // return
            for ret in &func.rets {
                function.instruction(&Instruction::LocalGet(*ret as u32));
            }
            function.instruction(&Instruction::Return);
            function.instruction(&Instruction::End);

            let func_idx = FuncIndex {
                func_idx: self.func_count,
                elem_idx: self.element_funcs.len() as u32 + self.element_offset as u32,
            };
            self.func_indices.insert(i, func_idx);

            self.functions.function(type_index);
            self.code.function(&function);
            self.element_funcs.push(self.func_count);
            self.func_count += 1;
        }

        let mut elements = ElementSection::new();
        elements.active(
            Some(0),
            &ConstExpr::i32_const(self.element_offset as i32),
            Elements::Functions(Cow::Borrowed(&self.element_funcs)),
        );

        let start = StartSection {
            function_index: self.func_indices[&ir.entry].func_idx,
        };

        let mut module = Module::new();
        module
            .section(&self.types)
            .section(&self.imports)
            .section(&self.functions)
            .section(&self.tables)
            .section(&self.globals)
            .section(&start)
            .section(&elements)
            .section(&self.code);

        module
    }

    fn gen_stat(&mut self, function: &mut Function, locals: &Vec<ir::Type>, stat: &ir::Stat) {
        match stat {
            ir::Stat::If(cond, then_stat, else_stat) => {
                function.instruction(&Instruction::LocalGet(*cond as u32));
                function.instruction(&Instruction::If(BlockType::Empty));
                for stat in then_stat {
                    self.gen_stat(function, locals, stat);
                }
                function.instruction(&Instruction::Else);
                for stat in else_stat {
                    self.gen_stat(function, locals, stat);
                }
                function.instruction(&Instruction::End);
            }
            ir::Stat::Expr(result, expr) => {
                self.gen_expr(function, locals, expr);
                if let Some(result) = result {
                    function.instruction(&Instruction::LocalSet(*result as u32));
                } else {
                    function.instruction(&Instruction::Drop);
                }
            }
        }
    }

    fn gen_expr(&mut self, function: &mut Function, locals: &Vec<ir::Type>, expr: &ir::Expr) {
        match expr {
            ir::Expr::Bool(b) => {
                function.instruction(&Instruction::I32Const(if *b { 1 } else { 0 }));
            }
            ir::Expr::Int(i) => {
                function.instruction(&Instruction::I32Const(*i));
            }
            ir::Expr::String(s) => {
                let bs = s.as_bytes();

                self.gen_malloc(function, 4 + bs.len() as u32);
                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
                function.instruction(&Instruction::I32Const(bs.len() as i32));
                function.instruction(&Instruction::I32Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                for (i, b) in bs.iter().enumerate() {
                    function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
                    function.instruction(&Instruction::I32Const(*b as i32));
                    function.instruction(&Instruction::I32Store8(MemArg {
                        align: 0,
                        offset: 4 + i as u64,
                        memory_index: 0,
                    }));
                }

                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
            }
            ir::Expr::StringToSymbol(s) => {
                function.instruction(&Instruction::LocalGet(*s as u32));
                function.instruction(&Instruction::Call(self.string_to_symbol_func));
            }
            ir::Expr::Nil => {
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::Cons(car, cdr) => {
                self.gen_malloc(function, 16);

                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
                function.instruction(&Instruction::LocalGet(*car as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
                function.instruction(&Instruction::LocalGet(*cdr as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 8,
                    memory_index: 0,
                }));

                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
            }
            ir::Expr::CreateMutCell => {
                self.gen_malloc(function, 8);
                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
            }
            ir::Expr::DerefMutCell(cell) => {
                function.instruction(&Instruction::LocalGet(*cell as u32));
                function.instruction(&Instruction::I64Load(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));
            }
            ir::Expr::SetMutCell(cell, val) => {
                function.instruction(&Instruction::LocalGet(*cell as u32));
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));
                function.instruction(&Instruction::LocalGet(*val as u32));
            }
            ir::Expr::Closure(envs, func) => {
                let sizes = envs
                    .iter()
                    .map(|env| Self::type_size(locals[*env]))
                    .collect::<Vec<_>>();
                let env_offsets = sizes
                    .iter()
                    .scan(0, |sum, size| {
                        let offset = *sum;
                        *sum += size;
                        Some(offset)
                    })
                    .collect::<Vec<_>>();
                let envs_size = sizes.iter().sum::<u32>();

                self.gen_malloc(function, 4 + envs_size);

                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
                function.instruction(&Instruction::I32Const(
                    self.func_indices[func].elem_idx as i32,
                ));
                function.instruction(&Instruction::I32Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                for (i, env) in envs.iter().enumerate() {
                    function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
                    function.instruction(&Instruction::LocalGet(*env as u32));
                    match sizes[i] {
                        // TODO: ref typeなどに対応
                        4 => {
                            function.instruction(&Instruction::I32Store(MemArg {
                                align: 2,
                                offset: 4 + env_offsets[i] as u64,
                                memory_index: 0,
                            }));
                        }
                        8 => {
                            function.instruction(&Instruction::I64Store(MemArg {
                                align: 2,
                                offset: 4 + env_offsets[i] as u64,
                                memory_index: 0,
                            }));
                        }
                        _ => {
                            panic!("unsupported size");
                        }
                    }
                }

                function.instruction(&Instruction::GlobalGet(self.malloc_tmp_global));
            }
            ir::Expr::CallClosure(closure, args) => {
                for arg in args {
                    function.instruction(&Instruction::LocalGet(*arg as u32));
                }

                function.instruction(&Instruction::LocalGet(*closure as u32));
                function.instruction(&Instruction::I32Load(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                function.instruction(&Instruction::CallIndirect {
                    type_index: self.func_type(&WasmFuncType::from_ir(ir::FuncType {
                        args: args.iter().map(|arg| locals[*arg]).collect(),
                        rets: vec![ir::Type::Boxed],
                    })),
                    table_index: 0,
                });
            }
            ir::Expr::Move(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
            }
            ir::Expr::Unbox(_typ, val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I32WrapI64);
            }
            ir::Expr::Box(typ, val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::valtype_to_bit_pattern(*typ)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::ClosureEnv(env_types, closure, env_index) => {
                let env_sizes = env_types
                    .iter()
                    .map(|ty| Self::type_size(*ty))
                    .collect::<Vec<_>>();
                let env_offsets = env_sizes
                    .iter()
                    .scan(0, |sum, size| {
                        let offset = *sum;
                        *sum += size;
                        Some(offset)
                    })
                    .collect::<Vec<_>>();

                function.instruction(&Instruction::LocalGet(*closure as u32));
                match env_sizes[*env_index] {
                    // TODO: ref typeなどに対応
                    4 => {
                        function.instruction(&Instruction::I32Load(MemArg {
                            align: 2,
                            offset: 4 + env_offsets[*env_index] as u64,
                            memory_index: 0,
                        }));
                    }
                    8 => {
                        function.instruction(&Instruction::I64Load(MemArg {
                            align: 2,
                            offset: 4 + env_offsets[*env_index] as u64,
                            memory_index: 0,
                        }));
                    }
                    _ => {
                        panic!("unsupported size");
                    }
                }
            }
            ir::Expr::GlobalGet(global) => {
                function.instruction(&Instruction::GlobalGet(
                    *self.global_to_index.get(global).unwrap(),
                ));
                function.instruction(&Instruction::I64Load(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));
            }
            ir::Expr::GlobalSet(global, val) => {
                function.instruction(&Instruction::GlobalGet(
                    *self.global_to_index.get(global).unwrap(),
                ));
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));
                function.instruction(&Instruction::LocalGet(*val as u32));
            }
            ir::Expr::Error(_) => {
                function.instruction(&Instruction::Unreachable);
                function.instruction(&Instruction::I32Const(0)); // TODO: これなくても型エラーにならない気がする
            }
            ir::Expr::Builtin(builtin, args) => {
                for arg in args {
                    function.instruction(&Instruction::LocalGet(*arg as u32));
                }
                self.gen_builtin(*builtin, function);
            }
            ir::Expr::GetBuiltin(builtin) => {
                function.instruction(&Instruction::GlobalGet(
                    *self.builtin_to_global.get(builtin).unwrap(),
                ));
                function.instruction(&Instruction::I64Load(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));
            }
            ir::Expr::SetBuiltin(builtin, val) => {
                function.instruction(&Instruction::GlobalGet(
                    *self.builtin_to_global.get(builtin).unwrap(),
                ));
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));
                function.instruction(&Instruction::LocalGet(*val as u32));
            }
            ir::Expr::InitGlobal(global_id) => {
                function.instruction(&Instruction::I32Const(*global_id as i32));
                function.instruction(&Instruction::Call(self.get_global_func));
                function.instruction(&Instruction::GlobalSet(
                    *self.global_to_index.get(global_id).unwrap(),
                ));
                function.instruction(&Instruction::I32Const(0));
            }
            ir::Expr::InitBuiltin(builtin) => {
                function.instruction(&Instruction::I32Const(builtin.id()));
                function.instruction(&Instruction::Call(self.get_builtin_func));
                function.instruction(&Instruction::GlobalSet(
                    *self.builtin_to_global.get(builtin).unwrap(),
                ));
                function.instruction(&Instruction::I32Const(0));
            }
        }
    }

    fn gen_malloc(&mut self, function: &mut Function, size: u32) {
        function.instruction(&Instruction::I32Const(size as i32));
        function.instruction(&Instruction::Call(self.malloc_func));
        function.instruction(&Instruction::GlobalSet(self.malloc_tmp_global));
    }

    fn valtype_to_type_id(typ: ir::ValType) -> u8 {
        match typ {
            ir::ValType::Bool => 0b0010,
            ir::ValType::Int => 0b0011,
            ir::ValType::String => 0b0101,
            ir::ValType::Symbol => 0b0111,
            ir::ValType::Nil => 0b0001,
            ir::ValType::Cons => 0b0100,
            ir::ValType::Closure => 0b0110,
        }
    }

    fn valtype_to_bit_pattern(typ: ir::ValType) -> i64 {
        let type_id = Self::valtype_to_type_id(typ);
        (((1 << 12) - 1) << 52) | (type_id as i64) << 48
    }

    fn convert_type(ty: ir::Type) -> ValType {
        match ty {
            ir::Type::Boxed => ValType::I64,
            ir::Type::MutCell => ValType::I32,
            ir::Type::Val(_) => ValType::I32,
        }
    }

    fn type_size(ty: ir::Type) -> u32 {
        match ty {
            ir::Type::Boxed => 8,
            ir::Type::MutCell => 4,
            ir::Type::Val(_) => 4,
        }
    }

    fn gen_builtin(&self, builtin: ast::Builtin, function: &mut Function) {
        match builtin {
            ast::Builtin::Display => {
                function.instruction(&Instruction::Call(self.dump_func));
                function.instruction(&Instruction::I32Const(0));
            }
            ast::Builtin::Add => {
                function.instruction(&Instruction::I32Add);
            }
        }
    }
}
