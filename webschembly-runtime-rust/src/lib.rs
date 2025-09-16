#![feature(ptr_as_ref_unchecked, allocator_api, slice_ptr_get)]
use core::cell::RefCell;
use rustc_hash::FxHashMap;
use std::vec;
mod logger;
use std::alloc::{Allocator, Global, Layout};
use std::ptr::NonNull;
mod env;
mod runtime;

#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: i32) -> i32 {
    let total_size = size as usize + std::mem::size_of::<usize>();
    let layout = Layout::from_size_align(total_size, std::mem::align_of::<usize>()).unwrap();
    let ptr = Global.allocate(layout).unwrap();
    let raw_ptr = ptr.as_mut_ptr() as *mut usize;
    unsafe {
        *raw_ptr = size as usize;
    }
    unsafe { raw_ptr.add(1) as *mut u8 as i32 }
}

#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(ptr: i32) {
    let ptr = ptr as *mut u8;
    let size_ptr = unsafe { (ptr as *mut usize).offset(-1) };
    let size = unsafe { *size_ptr };
    let layout = Layout::from_size_align(
        size + std::mem::size_of::<usize>(),
        std::mem::align_of::<usize>(),
    )
    .unwrap();
    unsafe { Global.deallocate(NonNull::new_unchecked(size_ptr as *mut u8), layout) };
}
thread_local!(
    static SYMBOL_MANAGER: RefCell<SymbolManager> = RefCell::new(SymbolManager::new());
);

#[unsafe(no_mangle)]
pub extern "C" fn _string_to_symbol(s_ptr: i32, s_buf: i32) -> i32 {
    let mut s = vec![0u8; s_buf as usize];
    unsafe {
        std::ptr::copy_nonoverlapping(s_ptr as *const u8, s.as_mut_ptr(), s_buf as usize);
    }
    SYMBOL_MANAGER.with(|symbol_manager| symbol_manager.borrow_mut().string_to_symbol(s))
}

struct SymbolManager {
    symbol_to_bytes: FxHashMap<usize, Vec<u8>>,
    bytes_to_symbol: FxHashMap<Vec<u8>, usize>,
    symbol_id: usize,
}

impl SymbolManager {
    fn new() -> Self {
        Self {
            symbol_to_bytes: FxHashMap::default(),
            bytes_to_symbol: FxHashMap::default(),
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

thread_local!(
    static COMPILER: RefCell<webschembly_compiler::compiler::Compiler> = RefCell::new(
        webschembly_compiler::compiler::Compiler::new(webschembly_compiler::compiler::Config {
            enable_jit: true,
            enable_split_bb: true,
        }),
    );
);

// const STDIN_FD: i32 = 0;
const STDOUT_FD: i32 = 1;
const STDERR_FD: i32 = 2;

fn load_src_inner(src: String, is_stdlib: bool) {
    let result = COMPILER.with(|compiler| {
        let mut compiler = compiler.borrow_mut();
        compiler.compile_module(&src, is_stdlib).map(|module| {
            let wasm = webschembly_compiler::wasm_generator::generate(module);
            let ir = if cfg!(debug_assertions) {
                let ir = format!("{}", module.display());
                Some(ir.into_bytes())
            } else {
                None
            };
            (wasm, ir)
        })
    });

    match result {
        Ok((wasm, ir)) => unsafe {
            env::js_instantiate(
                wasm.as_ptr() as i32,
                wasm.len() as i32,
                ir.as_ref().map(|ir| ir.as_ptr() as i32).unwrap_or(0),
                ir.as_ref().map(|ir| ir.len() as i32).unwrap_or(0),
                1,
            )
        },
        Err(err) => {
            let error_msg = format!("{}\n", err);
            WRITERS.with(|writers| {
                get_writer(&mut writers.borrow_mut(), STDERR_FD).write_buf(error_msg.as_bytes())
            });
            unsafe {
                runtime::throw_webassembly_exception();
            }
        }
    }
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
    let src = String::from_utf8(bytes).unwrap();
    load_src_inner(src, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    log::set_logger(&logger::WasmLogger).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
}

#[derive(Debug)]
struct WasmWriter {
    fd: i32,
    buf: Vec<u8>,
}

impl WasmWriter {
    fn new(fd: i32) -> Self {
        Self {
            fd,
            buf: Vec::new(),
        }
    }

    fn write_char(&mut self, c: i32) {
        let c = char::from_u32(u32::from_le_bytes(c.to_le_bytes())).unwrap_or('?');
        let len = c.len_utf8();
        let mut bytes = [0; 4];
        c.encode_utf8(&mut bytes);
        self.buf.extend_from_slice(&bytes[..len]);
        if c == '\n' {
            self.flush();
        } else {
            self.flush_if_needed();
        }
    }

    fn write_buf(&mut self, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
        if buf.iter().any(|&c| c == b'\n') {
            self.flush();
        } else {
            self.flush_if_needed();
        }
    }

    fn flush(&mut self) {
        let ptr = self.buf.as_ptr();
        let len = self.buf.len() as i32;
        unsafe {
            env::js_write_buf(self.fd, ptr as i32, len);
        }
        self.buf.clear();
    }

    fn flush_if_needed(&mut self) {
        if self.buf.len() > 1024 {
            self.flush();
        }
    }
}

thread_local!(
    static WRITERS: RefCell<FxHashMap<i32, WasmWriter>> = RefCell::new(FxHashMap::default());
);

fn get_writer(writers: &mut FxHashMap<i32, WasmWriter>, fd: i32) -> &mut WasmWriter {
    writers.entry(fd).or_insert_with(|| WasmWriter::new(fd))
}

#[unsafe(no_mangle)]
pub extern "C" fn write_char_fd(fd: i32, c: i32) {
    WRITERS.with(|writers| get_writer(&mut writers.borrow_mut(), fd).write_char(c));
}

// TODO: Rustのコード生成の都合で一旦
#[unsafe(no_mangle)]
pub extern "C" fn write_char(c: i32) {
    write_char_fd(STDOUT_FD, c);
}

#[unsafe(no_mangle)]
pub extern "C" fn write_buf(fd: i32, buf_ptr: i32, buf_len: i32) {
    let buf_ptr: *const u8 = buf_ptr as *const u8;
    let buf = unsafe { std::slice::from_raw_parts(buf_ptr, buf_len as usize) };
    WRITERS.with(|writers| get_writer(&mut writers.borrow_mut(), fd).write_buf(buf));
}

#[unsafe(no_mangle)]
pub extern "C" fn flush_all() {
    WRITERS.with(|writers| {
        for writer in writers.borrow_mut().values_mut() {
            writer.flush();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn cleanup() {
    flush_all();
}

#[unsafe(no_mangle)]
pub extern "C" fn _int_to_string(i: i64) -> i64 {
    let s = i.to_string();
    let s = s.as_bytes();
    let s_ptr = unsafe { malloc(s.len() as i32) };
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), s_ptr as *mut u8, s.len());
    }

    let s_len = s.len() as i32;
    cons_tuple_i32(s_ptr, s_len)
}

fn cons_tuple_i32(a: i32, b: i32) -> i64 {
    // rustではmultivalueが使えないので、(i32, i32) を i64 として表す
    let a = a.to_le_bytes();
    let b = b.to_le_bytes();
    let mut buf = [0; 8];
    buf[..4].copy_from_slice(&a);
    buf[4..].copy_from_slice(&b);
    i64::from_le_bytes(buf)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_global_id(buf_ptr: i32, buf_len: i32) -> i32 {
    let buf_ptr = buf_ptr as *const u8;
    let mut bytes = Vec::with_capacity(buf_len as usize);
    for i in 0..buf_len {
        unsafe {
            bytes.push(*buf_ptr.offset(i as isize));
        }
    }
    let name = String::from_utf8(bytes).unwrap();
    COMPILER.with(|compiler| {
        let compiler = compiler.borrow();
        compiler.get_global_id(&name).unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn instantiate_module(module_id: i32) -> i32 {
    let (wasm, ir) = COMPILER.with(|compiler| {
        let compiler = compiler.borrow();
        let module = compiler
            .instantiate_module(webschembly_compiler::ir::ModuleId::from(module_id as usize));
        let wasm = webschembly_compiler::wasm_generator::generate(module);
        let ir = if cfg!(debug_assertions) {
            let ir = format!("{}", module.display());
            Some(ir.into_bytes())
        } else {
            None
        };
        (wasm, ir)
    });

    unsafe {
        env::js_instantiate(
            wasm.as_ptr() as i32,
            wasm.len() as i32,
            ir.as_ref().map(|ir| ir.as_ptr() as i32).unwrap_or(0),
            ir.as_ref().map(|ir| ir.len() as i32).unwrap_or(0),
            0,
        )
    }

    0
}
