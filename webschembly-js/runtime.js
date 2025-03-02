export function createRuntime(
  { exit, logger, runtimeBuf, writeBuf },
  { exitWhenException = true, printEvalResult = false }
) {
  const runtimeImportObjects = {
    js_instantiate: (bufPtr, bufSize) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufSize
      );
      logger.instantiate(buf);

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
      logger.log(s);
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
