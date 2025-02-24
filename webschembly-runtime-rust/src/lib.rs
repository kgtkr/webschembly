#![feature(ptr_as_ref_unchecked, allocator_api, slice_ptr_get)]
use core::cell::RefCell;
use std::collections::HashMap;
use std::vec;
use webschembly_compiler;
mod logger;
use std::alloc::{Allocator, Global, Layout};
use std::ptr::NonNull;

#[no_mangle]
pub unsafe extern "C" fn malloc(size: i32) -> i32 {
    let total_size = size as usize + std::mem::size_of::<usize>();
    let layout = Layout::from_size_align(total_size, std::mem::align_of::<usize>()).unwrap();
    let ptr = Global.allocate(layout).unwrap();
    let raw_ptr = ptr.as_mut_ptr() as *mut usize;
    *raw_ptr = size as usize;
    raw_ptr.add(1) as *mut u8 as i32
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: i32) {
    let ptr = ptr as *mut u8;
    let size_ptr = (ptr as *mut usize).offset(-1);
    let size = *size_ptr;
    let layout = Layout::from_size_align(
        size + std::mem::size_of::<usize>(),
        std::mem::align_of::<usize>(),
    )
    .unwrap();
    Global.deallocate(NonNull::new_unchecked(size_ptr as *mut u8), layout);
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
    fn write_buf(buf_ptr: i32, buf_len: i32);
}

#[derive(Debug)]
struct WasmWriter {
    buf: Vec<u8>,
}

impl WasmWriter {
    fn new() -> Self {
        Self { buf: Vec::new() }
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
            write_buf(ptr as i32, len);
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
    static WRITER: RefCell<WasmWriter> = RefCell::new(WasmWriter::new());
);

#[unsafe(no_mangle)]
pub extern "C" fn write_char(c: i32) {
    WRITER.with(|writer| writer.borrow_mut().write_char(c));
}

#[unsafe(no_mangle)]
pub extern "C" fn write_buf_(buf_ptr: i32, buf_len: i32) {
    let buf_ptr = buf_ptr as *const u8;
    let buf = unsafe { std::slice::from_raw_parts(buf_ptr, buf_len as usize) };
    WRITER.with(|writer| writer.borrow_mut().write_buf(buf));
}

#[unsafe(no_mangle)]
pub extern "C" fn _int_to_string(i: i64) -> i64 {
    let s = i.to_string();
    let s = s.as_bytes();
    let s_ptr = unsafe { malloc(s.len() as i32) };
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), s_ptr as *mut u8, s.len());
    }

    // rustではmultivalueが使えないので、(i32, i32) を i64 として返す
    let s_ptr = s_ptr as i32;
    let s_len = s.len() as i32;
    let s_ptr = s_ptr.to_le_bytes();
    let s_len = s_len.to_le_bytes();
    let mut buf = [0; 8];
    buf[..4].copy_from_slice(&s_ptr);
    buf[4..].copy_from_slice(&s_len);
    i64::from_le_bytes(buf)
}
