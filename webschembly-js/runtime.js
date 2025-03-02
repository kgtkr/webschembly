import * as fs from "fs";
import * as path from "path";

export function createRuntime({
  runtimeName = "untitled",
  exit = process.exit,
  exitWhenException = true,
  logDir = process.env.LOG_DIR || null,
  runtimeBuf = fs.readFileSync(process.env["WEBSCHEMBLY_RUNTIME"]),
  writeBuf = (fd, buf) => {
    switch (fd) {
      case 1:
        process.stdout.write(buf);
        break;
      case 2:
        process.stderr.write(buf);
        break;
      default:
        throw new Error(`Unsupported file descriptor: ${fd}`);
    }
  },
  printEvalResult = false,
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
    js_instantiate: (bufPtr, bufSize) => {
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

      const result = instance.exports.start();
      if (printEvalResult) {
        runtimeInstance.exports.print_for_repl(result);
      }
    },
    js_webschembly_log: (bufPtr, bufLen) => {
      const s = new TextDecoder().decode(
        new Uint8Array(runtimeInstance.exports.memory.buffer, bufPtr, bufLen)
      );
      if (logFile !== null) {
        fs.writeSync(logFile, s + "\n");
      }
    },
    js_write_buf: (fd, bufPtr, bufLen) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufLen
      );
      writeBuf(1, buf);
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
        if (e instanceof WebAssembly.Exception) {
          if (e.is(runtimeInstance.exports.WEBSCHEMBLY_EXCEPTION)) {
            if (exitWhenException) {
              exit(1);
            }
          } else {
            throw e;
          }
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
