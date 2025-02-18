use std::collections::HashMap;

use super::ir;
use std::borrow::Cow;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, ElementSection, Elements, EntityType, Function,
    FunctionSection, ImportSection, Instruction, MemArg, MemoryType, Module, RefType, StartSection,
    TableSection, TableType, TypeSection, ValType,
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

#[derive(Debug)]
pub struct ModuleGenerator {
    func_to_type_index: HashMap<WasmFuncType, u32>,
    type_count: u32,
    func_count: u32,
    // runtime functions
    malloc_func: u32,
    dump_func: u32,
    string_to_symbol_func: u32,
    temp_local: u32,
    // wasm section
    imports: ImportSection,
    types: TypeSection,
    functions: FunctionSection,
    elements: ElementSection,
    tables: TableSection,
    code: CodeSection,
}

impl ModuleGenerator {
    pub fn new() -> Self {
        Self {
            func_to_type_index: HashMap::new(),
            type_count: 0,
            func_count: 0,
            malloc_func: 0,
            dump_func: 0,
            string_to_symbol_func: 0,
            // TODO: 0はenvで一度しか使われないので、一時変数として使っているが、危ないので修正
            temp_local: 0,
            imports: ImportSection::new(),
            types: TypeSection::new(),
            functions: FunctionSection::new(),
            elements: ElementSection::new(),
            tables: TableSection::new(),
            code: CodeSection::new(),
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

        let runtime_func_count = self.func_count;

        let mut element_functions = vec![];
        for func in &ir.funcs {
            let type_index = self.func_type(&WasmFuncType::from_ir(func.func_type()));
            self.functions.function(type_index);

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
            self.gen_stat(&mut function, &func.locals, &func.body);

            // return
            for ret in &func.rets {
                function.instruction(&Instruction::LocalGet(*ret as u32));
            }
            function.instruction(&Instruction::Return);
            function.instruction(&Instruction::End);

            self.code.function(&function);

            element_functions.push(self.func_count);
            self.func_count += 1;
        }

        self.tables.table(TableType {
            element_type: RefType::FUNCREF,
            minimum: element_functions.len() as u64,
            maximum: None,
            table64: false,
            shared: false,
        });

        self.elements.active(
            Some(0),
            &ConstExpr::i32_const(0),
            Elements::Functions(Cow::Borrowed(&element_functions)),
        );

        let start = StartSection {
            // TODO: ir func index -> wasm func indexのMapを作る
            function_index: ir.entry as u32 + runtime_func_count,
        };

        let mut module = Module::new();
        module
            .section(&self.types)
            .section(&self.imports)
            .section(&self.functions)
            .section(&self.tables)
            .section(&start)
            .section(&self.elements)
            .section(&self.code);

        module
    }

    fn gen_stat(&mut self, function: &mut Function, locals: &Vec<ir::Type>, stat: &ir::Stat) {
        match stat {
            ir::Stat::If(cond, then_stat, else_stat) => {
                function.instruction(&Instruction::LocalGet(*cond as u32));
                function.instruction(&Instruction::If(BlockType::Empty));
                self.gen_stat(function, locals, then_stat);
                function.instruction(&Instruction::Else);
                self.gen_stat(function, locals, else_stat);
                function.instruction(&Instruction::End);
            }
            ir::Stat::Begin(stats) => {
                for stat in stats {
                    self.gen_stat(function, locals, stat);
                }
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
                function.instruction(&Instruction::LocalSet(self.temp_local));

                function.instruction(&Instruction::LocalGet(self.temp_local));
                function.instruction(&Instruction::I32Const(bs.len() as i32));
                function.instruction(&Instruction::I32Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                for (i, b) in bs.iter().enumerate() {
                    function.instruction(&Instruction::LocalGet(self.temp_local));
                    function.instruction(&Instruction::I32Const(*b as i32));
                    function.instruction(&Instruction::I32Store8(MemArg {
                        align: 0,
                        offset: 4 + i as u64,
                        memory_index: 0,
                    }));
                }

                function.instruction(&Instruction::LocalGet(self.temp_local));
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
                function.instruction(&Instruction::LocalSet(self.temp_local));

                function.instruction(&Instruction::LocalGet(self.temp_local));
                function.instruction(&Instruction::LocalGet(*car as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                function.instruction(&Instruction::LocalGet(self.temp_local));
                function.instruction(&Instruction::LocalGet(*cdr as u32));
                function.instruction(&Instruction::I64Store(MemArg {
                    align: 2,
                    offset: 8,
                    memory_index: 0,
                }));

                function.instruction(&Instruction::LocalGet(self.temp_local));
            }
            ir::Expr::Closure(envs, func) => {
                self.gen_malloc(function, 4 + 8 * envs.len() as u32);
                function.instruction(&Instruction::LocalSet(self.temp_local));

                function.instruction(&Instruction::LocalGet(self.temp_local));
                function.instruction(&Instruction::I32Const(*func as i32));
                function.instruction(&Instruction::I32Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                for (i, env) in envs.iter().enumerate() {
                    function.instruction(&Instruction::LocalGet(self.temp_local));
                    function.instruction(&Instruction::LocalGet(*env as u32));
                    function.instruction(&Instruction::I64Store(MemArg {
                        align: 2,
                        offset: 4 + i as u64 * 8,
                        memory_index: 0,
                    }));
                }

                function.instruction(&Instruction::LocalGet(self.temp_local));
            }
            ir::Expr::Call(closure, args) => {
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
            ir::Expr::UnboxBool(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I32WrapI64);
            }
            ir::Expr::UnboxClosure(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I32WrapI64);
            }
            ir::Expr::BoxBool(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0010)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::BoxInt(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0011)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::BoxString(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0101)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::BoxSymbol(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0111)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::BoxNil(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0001)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::BoxCons(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0100)));
                function.instruction(&Instruction::I64Or);
            }
            ir::Expr::BoxClosure(val) => {
                function.instruction(&Instruction::LocalGet(*val as u32));
                function.instruction(&Instruction::I64ExtendI32U);
                function.instruction(&Instruction::I64Const(Self::gen_box_bit_pattern(0b0110)));
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
        }
    }

    fn gen_malloc(&mut self, function: &mut Function, size: u32) {
        function.instruction(&Instruction::I32Const(size as i32));
        function.instruction(&Instruction::Call(self.malloc_func));
    }

    fn gen_box_bit_pattern(type_id: u8) -> i64 {
        (((1 << 12) - 1) << 52) | (type_id as i64) << 48
    }

    fn convert_type(ty: ir::Type) -> ValType {
        match ty {
            ir::Type::Boxed => ValType::I64,
            ir::Type::Bool => ValType::I32,
            ir::Type::Int => ValType::I32,
            ir::Type::String => ValType::I32,
            ir::Type::Symbol => ValType::I32,
            ir::Type::Nil => ValType::I32,
            ir::Type::Cons => ValType::I32,
            ir::Type::Closure => ValType::I32,
        }
    }
}
