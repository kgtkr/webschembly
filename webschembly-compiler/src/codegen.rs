use std::collections::HashMap;

use super::ir;
use std::borrow::Cow;
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
pub struct ModuleGenerator {
    func_to_type_index: HashMap<WasmFuncType, u32>,
    type_count: u32,
    func_count: u32,
    global_count: u32,
    // runtime functions
    malloc_func: u32,
    dump_func: u32,
    string_to_symbol_func: u32,
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
}

impl ModuleGenerator {
    pub fn new() -> Self {
        Self {
            func_to_type_index: HashMap::new(),
            type_count: 0,
            func_count: 0,
            global_count: 0,
            malloc_func: 0,
            dump_func: 0,
            string_to_symbol_func: 0,
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

    pub fn gen(mut self, ir: &ir::Ir) -> Module {
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
                    val_type: ValType::I64,
                    mutable: true,
                    shared: false,
                },
                // TODO: nilか0で初期化
                &ConstExpr::i64_const(0),
            );
            self.global_count += 1;
            self.global_to_index.insert(global, global_index);
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
                elem_idx: self.element_funcs.len() as u32,
            };
            self.func_indices.insert(i, func_idx);

            self.functions.function(type_index);
            self.code.function(&function);
            self.element_funcs.push(self.func_count);
            self.func_count += 1;
        }

        self.tables.table(TableType {
            element_type: RefType::FUNCREF,
            minimum: self.element_funcs.len() as u64,
            maximum: None,
            table64: false,
            shared: false,
        });

        let mut elements = ElementSection::new();
        elements.active(
            Some(0),
            &ConstExpr::i32_const(0),
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
            ir::Expr::Closure(envs, func) => {
                self.gen_malloc(function, 4 + 8 * envs.len() as u32);

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
                    function.instruction(&Instruction::I64Store(MemArg {
                        align: 2,
                        offset: 4 + i as u64 * 8,
                        memory_index: 0,
                    }));
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
            ir::Expr::Dump(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::Call(self.dump_func));
                function.instruction(&Instruction::LocalGet(*val as u32));
            }
            ir::Expr::ClosureEnv(closure, env_index) => {
                function.instruction(&Instruction::LocalGet(*closure as u32));
                function.instruction(&Instruction::I64Load(MemArg {
                    align: 2,
                    offset: 4 + 8 * *env_index as u64,
                    memory_index: 0,
                }));
            }
            ir::Expr::GlobalGet(global) => {
                function.instruction(&Instruction::GlobalGet(
                    *self.global_to_index.get(global).unwrap(),
                ));
            }
            ir::Expr::GlobalSet(global, val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::GlobalSet(
                    *self.global_to_index.get(global).unwrap(),
                ));
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
            ir::Type::Val(_) => ValType::I32,
        }
    }
}
