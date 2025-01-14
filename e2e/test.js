// read ../target/wasm32-unknown-unknown/debug/webschembly_runtime.wasm
const fs = require("fs");
const runtimeInstance = new WebAssembly.Instance(
  new WebAssembly.Module(
    new Uint8Array(
      fs.readFileSync(
        "../target/wasm32-unknown-unknown/debug/webschembly_runtime.wasm"
      )
    )
  ),
  {}
);

const instance = new WebAssembly.Instance(
  new WebAssembly.Module(new Uint8Array(fs.readFileSync("./test.wasm"))),
  {
    runtime: runtimeInstance.exports,
  }
);
