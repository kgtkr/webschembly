#![feature(ptr_as_ref_unchecked)]
use core::cell::RefCell;
use std::collections::HashMap;
use webschembly_compiler;
mod logger;

thread_local!(
    static HEAP_MANAGER: RefCell<HeapManager> = RefCell::new(HeapManager::new());
);

#[unsafe(no_mangle)]
pub extern "C" fn malloc(size: i32) -> i32 {
    unsafe {
        HEAP_MANAGER.with(|heap_manager| {
            let mut heap_manager = heap_manager.borrow_mut();
            heap_manager.malloc(size) as i32
        })
    }
}

thread_local!(
    static SYMBOL_MANAGER: RefCell<SymbolManager> = RefCell::new(SymbolManager::new());
);

#[unsafe(no_mangle)]
pub extern "C" fn _string_to_symbol(string: i32) -> i32 {
    let string = unsafe { read_string(string) };
    SYMBOL_MANAGER.with(|symbol_manager| symbol_manager.borrow_mut().string_to_symbol(string))
}

unsafe fn read_string(string: i32) -> Vec<u8> {
    let len_ptr = string as *const [u8; 4];
    let buf_ptr = (string + 4) as *const u8;

    let len = i32::from_le_bytes(*len_ptr);
    let mut bytes = Vec::with_capacity(len as usize);
    for i in 0..len {
        bytes.push(*buf_ptr.offset(i as isize));
    }

    bytes
}

struct SymbolManager {
    symbol_to_bytes: HashMap<usize, Vec<u8>>,
    bytes_to_symbol: HashMap<Vec<u8>, usize>,
    symbol_id: usize,
}

impl SymbolManager {
    fn new() -> Self {
        Self {
            symbol_to_bytes: HashMap::new(),
            bytes_to_symbol: HashMap::new(),
            symbol_id: 0,
        }
    }

    fn string_to_symbol(&mut self, string: Vec<u8>) -> i32 {
        if let Some(symbol) = self.bytes_to_symbol.get(&string) {
            return *symbol as i32;
        }

        let symbol_id = self.symbol_id;
        self.symbol_to_bytes.insert(symbol_id, string.clone());
        self.bytes_to_symbol.insert(string, symbol_id);

        self.symbol_id += 1;

        symbol_id as i32
    }
}

const HEAP_SIZE: usize = 1024 * 8;

struct HeapManager {
    heap: [u8; HEAP_SIZE],
    offset: usize,
}

impl HeapManager {
    fn new() -> Self {
        Self {
            heap: [0; HEAP_SIZE],
            offset: 0,
        }
    }

    unsafe fn malloc(&mut self, size: i32) -> *const u8 {
        let offset = self.offset;
        self.offset += size as usize;

        self.heap[offset as usize..].as_ptr()
    }
}

thread_local!(
    static COMPILER: RefCell<webschembly_compiler::compiler::Compiler> =
        RefCell::new(webschembly_compiler::compiler::Compiler::new());
);

fn load_src_inner(src: String, is_stdlib: bool) {
    COMPILER.with(|compiler| {
        let mut compiler = compiler.borrow_mut();
        let wasm = compiler.compile(&src, is_stdlib).unwrap();
        unsafe { instantiate(wasm.as_ptr() as i32, wasm.len() as i32) };
        drop(wasm);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn load_stdlib() {
    let stdlib = webschembly_compiler::stdlib::generate_stdlib();
    load_src_inner(stdlib, true);
}

#[unsafe(no_mangle)]
pub extern "C" fn load_src(buf_ptr: i32, buf_len: i32) {
    let buf_ptr = buf_ptr as *const u8;
    let mut bytes = Vec::with_capacity(buf_len as usize);
    for i in 0..buf_len {
        unsafe {
            bytes.push(*buf_ptr.offset(i as isize));
        }
    }
    // TODO: free buf_ptr
    let src = String::from_utf8(bytes).unwrap();
    load_src_inner(src, false);
}

extern "C" {
    fn instantiate(buf_ptr: i32, buf_size: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    log::set_logger(&logger::WasmLogger).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
}

extern "C" {
    fn println(buf_ptr: i32, buf_len: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn _display(x: i64) {
    fn boxed_to_string(x: u64, s: &mut String) {
        let type_mask = ((1 << 4) - 1) << 48;
        let value_mask = (1 << 48) - 1;

        let type_id = (x & type_mask) >> 48;
        let value = (x & value_mask) as u32;

        match type_id {
            1 => s.push_str("()"),
            2 => s.push_str(if value == 0 { "#f" } else { "#t" }),
            3 => s.push_str(
                &i32::from_le_bytes({
                    let mut bytes = [0; 4];
                    bytes.copy_from_slice(&value.to_le_bytes()[..]);
                    bytes
                })
                .to_string(),
            ),
            4 => {
                let car = u64::from_le_bytes(unsafe {
                    let ptr = value as *const [u8; 8];

                    let mut bytes = [0; 8];
                    bytes.copy_from_slice(&(*ptr));
                    bytes
                });
                let cdr = u64::from_le_bytes(unsafe {
                    let ptr = (value + 8) as *const [u8; 8];

                    let mut bytes = [0; 8];
                    bytes.copy_from_slice(&(*ptr));
                    bytes
                });
                s.push('(');
                boxed_to_string(car, s);
                s.push_str(" . ");
                boxed_to_string(cdr, s);
                s.push(')');
            }
            5 => {
                let string = unsafe { read_string(value as i32) };
                let string = String::from_utf8_lossy(&string);
                s.push('"');
                s.push_str(&string);
                s.push('"');
            }
            6 => {
                s.push_str("<closure#");
                s.push_str(
                    &u32::from_le_bytes(unsafe {
                        let ptr = value as *const [u8; 4];

                        let mut bytes = [0; 4];
                        bytes.copy_from_slice(&(*ptr));
                        bytes
                    })
                    .to_string(),
                );
                s.push('>');
            }
            7 => {
                s.push_str("<symbol#");
                s.push_str(&value.to_string());
                s.push('>');
            }
            _ => {
                s.push_str("<unknown_type_id: ");
                s.push_str(&type_id.to_string());
                s.push_str(" ,");
                s.push_str(&value.to_string());
                s.push('>');
            }
        }
    }

    let mut s = String::new();
    boxed_to_string(x as u64, &mut s);

    let ptr = s.as_ptr();
    let len = s.len() as i32;
    unsafe {
        println(ptr as i32, len);
    }
    drop(s);
}
