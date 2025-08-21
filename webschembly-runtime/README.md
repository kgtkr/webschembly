# webschembly-runtime

WebAssembly runtime library for Webschembly

主にwebschembly-runtime-rustで実装されている機能をwasm gcのstructに変換する役割。`lib.wat` に実装されており、 `make webschembly_runtime.wasm` で `webschembly-runtime-rust`とリンクされる。
