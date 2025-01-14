use std::collections::HashMap;

use super::ir;
use std::borrow::Cow;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, ElementSection, Elements, EntityType, Function,
    FunctionSection, ImportSection, Instruction, MemArg, MemoryType, Module, RefType, TableSection,
    TableType, TypeSection, ValType,
};

#[derive(Debug)]
pub struct ModuleGenerator {
    args_to_type_index: HashMap<usize, u32>,
    type_count: u32,
    func_count: u32,
    // runtime functions
    malloc_func: u32,
    dump_func: u32,
    string_to_symbol_func: u32,
    temp_local: u32, // 0はenvで一度しか使われないので、一時変数として使う
}

impl ModuleGenerator {
    pub fn new() -> Self {
        Self {
            args_to_type_index: HashMap::new(),
            type_count: 0,
            func_count: 0,
            malloc_func: 0,
            dump_func: 0,
            string_to_symbol_func: 0,
            temp_local: 0,
        }
    }

    pub fn gen(mut self, ir: &ir::Ir) -> Module {
        let mut module = Module::new();

        let mut imports = ImportSection::new();
        let mut types = TypeSection::new();
        let mut functions = FunctionSection::new();
        let mut elements = ElementSection::new();
        let mut tables = TableSection::new();
        let mut code = CodeSection::new();

        imports.import(
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

        // malloc
        types.ty().function(vec![ValType::I32], vec![ValType::I32]);
        let malloc_type = self.type_count;
        self.type_count += 1;
        imports.import("runtime", "malloc", EntityType::Function(malloc_type));
        self.malloc_func = self.func_count;
        self.func_count += 1;

        // dump
        types.ty().function(vec![ValType::I64], vec![]);
        let dump_type = self.type_count;
        self.type_count += 1;
        imports.import("runtime", "dump", EntityType::Function(dump_type));
        self.dump_func = self.func_count;
        self.func_count += 1;

        // string_to_symbol
        types.ty().function(vec![ValType::I32], vec![ValType::I32]);
        let string_to_symbol_type = self.type_count;
        self.type_count += 1;
        imports.import(
            "runtime",
            "string_to_symbol",
            EntityType::Function(string_to_symbol_type),
        );

        let mut element_functions = vec![];
        for func in &ir.funcs {
            let type_index = if let Some(type_index) = self.args_to_type_index.get(&func.args) {
                *type_index
            } else {
                let mut params = vec![ValType::I32];
                for _ in 1..func.args {
                    params.push(ValType::I64);
                }

                types.ty().function(params, vec![ValType::I64]);
                let type_index = self.type_count;
                self.args_to_type_index.insert(func.args, type_index);
                self.type_count += 1;
                type_index
            };

            functions.function(type_index);

            let mut function = Function::new(
                func.locals
                    .iter()
                    .skip(func.args)
                    .map(|ty| {
                        let ty = self.convert_type(*ty);
                        (1, ty)
                    })
                    .collect::<Vec<_>>(),
            );

            // envs to locals
            for env in 0..func.envs {
                let local = func.args + env;

                function.instruction(&Instruction::LocalGet(0));
                function.instruction(&Instruction::I64Load(MemArg {
                    align: 2,
                    offset: 4 * env as u64,
                    memory_index: 0,
                }));
                function.instruction(&Instruction::LocalSet(local as u32));
            }

            // body
            self.gen_stat(&mut function, &func.body);

            // return
            function.instruction(&Instruction::LocalGet(func.ret as u32));
            function.instruction(&Instruction::Return);

            code.function(&function);

            element_functions.push(self.func_count);
            self.func_count += 1;
        }

        tables.table(TableType {
            element_type: RefType::FUNCREF,
            minimum: element_functions.len() as u64,
            maximum: None,
            table64: false,
            shared: false,
        });

        elements.active(
            Some(0),
            &ConstExpr::i32_const(42),
            Elements::Functions(Cow::Borrowed(&element_functions)),
        );

        module
            .section(&imports)
            .section(&types)
            .section(&functions)
            .section(&tables)
            .section(&elements)
            .section(&code);

        module
    }

    fn gen_stat(&mut self, function: &mut Function, stat: &ir::Stat) {
        match stat {
            ir::Stat::If(cond, then_stat, else_stat) => {
                function.instruction(&Instruction::LocalGet(*cond as u32));
                function.instruction(&Instruction::If(BlockType::Empty));
                self.gen_stat(function, then_stat);
                function.instruction(&Instruction::Else);
                self.gen_stat(function, else_stat);
                function.instruction(&Instruction::End);
            }
            ir::Stat::Begin(stats) => {
                for stat in stats {
                    self.gen_stat(function, stat);
                }
            }
            ir::Stat::Expr(result, expr) => {
                self.gen_expr(function, expr);
                if let Some(result) = result {
                    function.instruction(&Instruction::LocalSet(*result as u32));
                } else {
                    function.instruction(&Instruction::Drop);
                }
            }
        }
    }

    fn gen_expr(&mut self, function: &mut Function, expr: &ir::Expr) {
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
                function.instruction(&Instruction::I32Store(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                function.instruction(&Instruction::LocalGet(self.temp_local));
                function.instruction(&Instruction::LocalGet(*cdr as u32));
                function.instruction(&Instruction::I32Store(MemArg {
                    align: 2,
                    offset: 4,
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
                    function.instruction(&Instruction::I32Store(MemArg {
                        align: 2,
                        offset: 4 + i as u64 * 8,
                        memory_index: 0,
                    }));
                }

                function.instruction(&Instruction::LocalGet(self.temp_local));
            }
            ir::Expr::Call(closure, args) => {
                function.instruction(&Instruction::LocalGet(*closure as u32));
                function.instruction(&Instruction::I32Load(MemArg {
                    align: 2,
                    offset: 0,
                    memory_index: 0,
                }));

                for arg in args {
                    function.instruction(&Instruction::LocalGet(*arg as u32));
                }

                function.instruction(&Instruction::CallIndirect {
                    type_index: *self.args_to_type_index.get(&(args.len() + 1)).unwrap(),
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
        }
    }

    fn gen_malloc(&mut self, function: &mut Function, size: u32) {
        function.instruction(&Instruction::I32Const(size as i32));
        function.instruction(&Instruction::Call(self.malloc_func));
    }

    fn gen_box_bit_pattern(type_id: u8) -> i64 {
        (((1 << 12) - 1) << 48) | (type_id as i64) << 44
    }

    fn convert_type(&self, ty: ir::Type) -> ValType {
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
