import * as fs from "fs";
import * as path from "path";

export function createRuntime({
  runtimeName = "untitled",
  exit = process.exit,
  logDir = process.env.LOG_DIR || null,
  runtimeBuf = fs.readFileSync(process.env["WEBSCHEMBLY_RUNTIME"]),
  eprintln = console.error,
  writeBuf = process.stdout.write.bind(process.stdout),
}) {
  const logBasename = path.basename(runtimeName) + "-" + Date.now();
  let logFile = null;
  if (logDir !== null) {
    try {
      fs.mkdirSync(logDir);
    } catch (e) {
      if (e.code !== "EEXIST") {
        throw e;
      }
    }

    logFile = fs.openSync(path.join(logDir, logBasename + ".log"), "a");
  }
  let instantiateCount = 0;

  const runtimeImportObjects = {
    instantiate: (bufPtr, bufSize) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufSize
      );
      if (logDir !== null) {
        fs.writeFileSync(
          path.join(logDir, logBasename + "-" + instantiateCount + ".wasm"),
          buf
        );
      }
      instantiateCount++;

      const instance = new WebAssembly.Instance(
        new WebAssembly.Module(buf),
        importObject
      );

      instance.exports.start();
    },
    webschembly_log: (bufPtr, bufLen) => {
      const s = new TextDecoder().decode(
        new Uint8Array(runtimeInstance.exports.memory.buffer, bufPtr, bufLen)
      );
      if (logFile !== null) {
        fs.writeSync(logFile, s + "\n");
      }
    },
    write_buf: (bufPtr, bufLen) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufLen
      );
      writeBuf(buf);
    },
  };

  const runtimeInstance = new WebAssembly.Instance(
    new WebAssembly.Module(new Uint8Array(runtimeBuf)),
    {
      env: runtimeImportObjects,
    }
  );

  const importObject = {
    runtime: runtimeInstance.exports,
  };

  const errorHandle =
    (f) =>
    (...args) => {
      try {
        f(...args);
      } catch (e) {
        if (e instanceof WebAssembly.RuntimeError) {
          // エラーログに絶対パスなどが入るとsnapshot testに支障が出るため
          // TODO: 言語としてエラーメッセージを整備する
          eprintln(`${e.name}: ${e.message}`);
          exit(1);
        } else {
          throw e;
        }
      }
    };

  return {
    loadStdlib: errorHandle(() => {
      runtimeInstance.exports.load_stdlib();
    }),
    loadSrc: errorHandle((srcBuf) => {
      const srcBufPtr = runtimeInstance.exports.malloc(srcBuf.length);
      new Uint8Array(runtimeInstance.exports.memory.buffer).set(
        srcBuf,
        srcBufPtr
      );
      runtimeInstance.exports.load_src(srcBufPtr, srcBuf.length);
      runtimeInstance.exports.free(srcBufPtr);
    }),
    flushAll: () => {
      runtimeInstance.exports.flush_all();
    },
    cleanup: () => {
      runtimeInstance.exports.cleanup();
    },
  };
}
