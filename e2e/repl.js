const { getRuntime } = require("./runtime");
const fs = require("fs");

const srcName = "repl.scm";
const runtime = getRuntime(srcName);

const stdin = fs.openSync("/dev/stdin", "r");
runtime.exports.load_stdlib();

(async () => {
  for await (const chunk of process.stdin) {
    const srcBuf = new Uint8Array(chunk);
    const srcBufPtr = runtime.exports.malloc(srcBuf.length);
    new Uint8Array(runtime.exports.memory.buffer).set(srcBuf, srcBufPtr);

    runtime.exports.load_src(srcBufPtr, srcBuf.length);
  }
})();
