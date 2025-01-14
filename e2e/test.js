// read ../target/wasm32-unknown-unknown/debug/webschembly_runtime.wasm
const fs = require("fs");
const buf = fs.readFileSync(
  "../target/wasm32-unknown-unknown/debug/webschembly_runtime.wasm"
);
const wasm = new WebAssembly.Module(new Uint8Array(buf));
const instance = new WebAssembly.Instance(wasm, {});

console.log(instance.exports.memory);
console.log(instance.exports.malloc(100));
console.log(instance.exports.malloc(100));
