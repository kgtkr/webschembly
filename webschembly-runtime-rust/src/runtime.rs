#[link(wasm_import_module = "runtime")]
extern "C" {
    pub fn throw_webassembly_exception();
}
