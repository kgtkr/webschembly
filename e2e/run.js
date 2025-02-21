const fs = require("fs");
const { getRuntime } = require("./runtime");

const srcName = process.argv[2];
const runtime = getRuntime(srcName);

const srcBuf = new Uint8Array(fs.readFileSync(srcName));
const srcBufPtr = runtime.exports.malloc(srcBuf.length);
new Uint8Array(runtime.exports.memory.buffer).set(srcBuf, srcBufPtr);

try {
  runtime.exports.load_stdlib();
  runtime.exports.load_src(srcBufPtr, srcBuf.length);
} catch (e) {
  // エラーログに絶対パスなどが入るとsnapshot testに支障が出るため
  // TODO: 言語としてエラーメッセージを整備する
  console.error(e.name, e.message);
  process.exit(1);
}
