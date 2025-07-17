export function createRuntime(
  { exit, logger, runtimeModule, writeBuf },
  { exitWhenException = true, printEvalResult = false }
) {
  const runtimeImportObjects = {
    js_instantiate: (bufPtr, bufSize, fromSrc) => {
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
      if (printEvalResult && fromSrc !== 0) {
        const writeClosure = runtimeInstance.exports.get_global(
          writePtr,
          writeLen
        );
        const writeParams = runtimeInstance.exports.new_vector(1);
        runtimeInstance.exports.set_vector(writeParams, 0, result);
        runtimeInstance.exports.call_closure(writeClosure, writeParams);

        const newlineClosure = runtimeInstance.exports.get_global(
          newlinePtr,
          newlineLen
        );
        const newlineParams = runtimeInstance.exports.new_vector(0);
        runtimeInstance.exports.call_closure(newlineClosure, newlineParams);
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
      writeBuf(fd, buf);
    },
  };

  const runtimeInstance = new WebAssembly.Instance(runtimeModule, {
    env: runtimeImportObjects,
  });

  const importObject = {
    runtime: runtimeInstance.exports,
  };

  const errorHandle =
    (f) =>
    (...args) => {
      try {
        f(...args);
      } catch (e) {
        if (
          e instanceof WebAssembly.Exception &&
          e.is(runtimeInstance.exports.WEBSCHEMBLY_EXCEPTION)
        ) {
          if (exitWhenException) {
            exit(1);
          }
        } else {
          throw e;
        }
      }
    };

  function mallocString(s) {
    const buf = new TextEncoder().encode(s);
    const bufPtr = runtimeInstance.exports.malloc(buf.length);
    new Uint8Array(runtimeInstance.exports.memory.buffer).set(buf, bufPtr);
    return [bufPtr, buf.length];
  }

  const [writePtr, writeLen] = mallocString("write");
  const [newlinePtr, newlineLen] = mallocString("newline");

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
