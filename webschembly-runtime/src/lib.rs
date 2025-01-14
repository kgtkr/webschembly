use core::{
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
};
use std::collections::HashMap;

const HEAP_SIZE: usize = 1024 * 1024;
static mut HEAP: UnsafeCell<[u8; HEAP_SIZE]> = UnsafeCell::new([0; HEAP_SIZE]);
static mut HEAP_OFFSET: Cell<usize> = Cell::new(0);

static mut SYMBOL_INIT: Cell<bool> = Cell::new(false);
static mut SYMBOL_ID: Cell<usize> = Cell::new(0);
static mut SYMBOL_TO_BYTES: UnsafeCell<MaybeUninit<HashMap<usize, Vec<u8>>>> =
    UnsafeCell::new(MaybeUninit::uninit());
static mut BYTES_TO_SYMBOL: UnsafeCell<MaybeUninit<HashMap<Vec<u8>, usize>>> =
    UnsafeCell::new(MaybeUninit::uninit());

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

#[unsafe(no_mangle)]
pub extern "C" fn string_to_symbol(string: i32) -> i32 {
    unsafe {
        let symbol_init = SYMBOL_INIT.get();
        if !symbol_init {
            SYMBOL_TO_BYTES
                .get()
                .write(MaybeUninit::new(HashMap::new()));
            BYTES_TO_SYMBOL
                .get()
                .write(MaybeUninit::new(HashMap::new()));
            SYMBOL_INIT.set(true);
        }

        let len = *(string as *const i32);
        let mut bytes = Vec::with_capacity(len as usize);
        for i in 0..len {
            bytes.push(*((string + 4) as *const u8).offset(i as isize));
        }

        let symbol_to_bytes = SYMBOL_TO_BYTES.get().read().as_mut_ptr();
        let bytes_to_symbol = BYTES_TO_SYMBOL.get().read().as_mut_ptr();

        let symbol_id = SYMBOL_ID.get();

        if let Some(symbol) = (*BYTES_TO_SYMBOL.get()).assume_init_ref().get(&bytes) {
            return *symbol as i32;
        }

        (*SYMBOL_TO_BYTES.get())
            .assume_init_mut()
            .insert(symbol_id, bytes.clone());
        (*BYTES_TO_SYMBOL.get())
            .assume_init_mut()
            .insert(bytes, symbol_id);

        SYMBOL_ID.set(symbol_id + 1);

        symbol_id as i32
    }
}
