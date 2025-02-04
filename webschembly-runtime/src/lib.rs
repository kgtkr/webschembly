#![feature(ptr_as_ref_unchecked)]
use core::cell::RefCell;
use std::collections::HashMap;

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
