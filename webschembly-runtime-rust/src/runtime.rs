#[link(wasm_import_module = "runtime")]
unsafe extern "C" {
    pub fn throw_webassembly_exception();
}
