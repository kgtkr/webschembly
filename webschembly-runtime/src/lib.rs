#![feature(ptr_as_ref_unchecked)]
use core::cell::{Cell, RefCell, UnsafeCell};
use std::collections::HashMap;

const HEAP_SIZE: usize = 1024 * 1024;
static mut HEAP: UnsafeCell<[u8; HEAP_SIZE]> = UnsafeCell::new([0; HEAP_SIZE]);
static mut HEAP_OFFSET: Cell<usize> = Cell::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn malloc(size: i32) -> i32 {
    unsafe {
        let offset = HEAP_OFFSET.get();
        HEAP_OFFSET.set(offset + size as usize);

        offset as i32 + HEAP.get() as i32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dump(value: i64) {
    println!("{:b}", value);
}

thread_local!(
    static SYMBOL_MANAGER: RefCell<SymbolManager> = RefCell::new(SymbolManager::new());
);

#[unsafe(no_mangle)]
pub extern "C" fn string_to_symbol(string: i32) -> i32 {
    let string = unsafe { read_string(string) };
    SYMBOL_MANAGER.with(|symbol_manager| symbol_manager.borrow_mut().string_to_symbol(string))
}

unsafe fn read_string(string: i32) -> Vec<u8> {
    let len_ptr = string as *const i32;
    let buf_ptr = (string + 4) as *const u8;

    let len = *len_ptr;
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

        self.symbol_to_bytes.insert(self.symbol_id, string.clone());
        self.bytes_to_symbol.insert(string, self.symbol_id);

        self.symbol_id += 1;

        (self.symbol_id - 1) as i32
    }
}
