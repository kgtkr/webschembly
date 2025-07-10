import * as fs from "fs";
import { createRuntime } from "./runtime.js";
import { createNodeRuntimeEnv } from "./node-runtime-env.js";

const wasmName = process.argv[2];
if (!wasmName) {
  console.error("Usage: run-aot.js <wasm>");
  process.exit(1);
}

const runtimeEnv = createNodeRuntimeEnv({ runtimeModule: null });
const runtimeImportObjects = {
  js_instantiate: (bufPtr, bufSize, fromSrc) => {
    throw new Error(
      "js_instantiate is not supported in run-aot.js. Use run.js instead."
    );
  },
  js_webschembly_log: (bufPtr, bufLen) => {
    const s = new TextDecoder().decode(
      new Uint8Array(wasmInstance.exports.memory.buffer, bufPtr, bufLen)
    );
    runtimeEnv.logger.log(s);
  },
  js_write_buf: (fd, bufPtr, bufLen) => {
    const buf = new Uint8Array(
      wasmInstance.exports.memory.buffer,
      bufPtr,
      bufLen
    );
    runtimeEnv.writeBuf(fd, buf);
  },
};

const wasmBuf = new Uint8Array(fs.readFileSync(wasmName));
const wasmModule = new WebAssembly.Module(wasmBuf);
const wasmInstance = new WebAssembly.Instance(wasmModule, {
  env: runtimeImportObjects,
});

// reverseしないと動かない(runtimeが後にexportされているため)
for (const key of Object.keys(wasmInstance.exports).reverse()) {
  if (key.startsWith("start")) {
    wasmInstance.exports[key]();
  }
}
