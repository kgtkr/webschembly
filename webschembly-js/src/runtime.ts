export type RuntimeEnv = {
  exit: (code: number) => void;
  logger: RuntimeLogger;
  loadRuntimeModule: () => Promise<WebAssembly.Module>;
  writeBuf: (fd: number, buf: Uint8Array) => void;
};

export type RuntimeConfig = {
  exitWhenException?: boolean;
  printEvalResult?: boolean;
  compilerConfig?: CompilerConfig;
};

export type CompilerConfig = {
  enableJit?: boolean;
  enableJitOptimization?: boolean;
  enableJitSmallBlockFusion?: boolean;
  enableJitLargeBlockFusion?: boolean;
};

export function compilerConfigToString(config: CompilerConfig): string {
  return Object.entries(config)
    .filter(([, v]) => v !== undefined)
    .map(([k, v]) => `${k}=${v}`)
    .join(",");
}

export type RuntimeLogger = {
  log: (s: string) => void;
  instantiate: (buf: Uint8Array, ir: string | null) => void;
};

export type Runtime = {
  loadStdlib: () => void;
  loadSrc: (srcBuf: Uint8Array) => void;
  flushAll: () => void;
  cleanup: () => void;
  mallocString: (s: string) => [ptr: number, len: number];
  instance: TypedWebAssemblyInstance<RuntimeExports>;
  getGlobal(name: string): SchemeValue;
};

export type SchemeValue = { __tagSchemeValueBrand: any };

export type RuntimeImportsEnv = {
  js_instantiate: (
    bufPtr: number,
    bufSize: number,
    irBufPtr: number,
    irBufSize: number,
    fromSrc: number,
  ) => void;
  js_webschembly_log: (bufPtr: number, bufLen: number) => void;
  js_write_buf: (fd: number, bufPtr: number, bufLen: number) => void;
};

export type RuntimeImports = {
  env: RuntimeImportsEnv;
};

export type RuntimeExports = {
  memory: WebAssembly.Memory;
  WEBSCHEMBLY_EXCEPTION: WebAssembly.ExceptionTag;
  get_global_id: (namePtr: number, nameLen: number) => number;
  new_args: (elemSize: number) => SchemeValue;
  set_args: (args: SchemeValue, index: number, value: SchemeValue) => void;
  call_closure: (closure: SchemeValue, args: SchemeValue) => SchemeValue;
  malloc: (size: number) => number;
  free: (ptr: number) => void;
  load_stdlib: () => void;
  load_src: (srcPtr: number, srcLen: number) => void;
  flush_all: () => void;
  cleanup: () => void;
  init: () => void;
  compiler_config_enable_jit: (enable: number) => void;
  compiler_config_enable_jit_optimization: (enable: number) => void;
  compiler_config_enable_jit_block_fusion: (enable: number) => void;
  compiler_config_enable_jit_large_block_fusion: (enable: number) => void;
};

export type ModuleImports = {
  runtime: RuntimeExports;
  dynamic: Record<string, WebAssembly.ExportValue>;
};

export type ModuleExports = {
  start: () => SchemeValue;
};

export type TypedWebAssemblyInstance<Exports> = WebAssembly.Instance & {
  exports: Exports;
};

export async function createRuntime(
  { exit, logger, loadRuntimeModule, writeBuf }: RuntimeEnv,
  {
    exitWhenException = true,
    printEvalResult = false,
    compilerConfig,
  }: RuntimeConfig,
): Promise<Runtime> {
  const dynamic: Record<string, WebAssembly.ExportValue> = {};
  const runtimeImportObjects: RuntimeImportsEnv = {
    js_instantiate: (bufPtr, bufSize, irBufPtr, irBufSize, fromSrc) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufSize,
      );
      const ir = irBufPtr === 0
        ? null
        : new TextDecoder().decode(
          new Uint8Array(
            runtimeInstance.exports.memory.buffer,
            irBufPtr,
            irBufSize,
          ),
        );

      logger.instantiate(buf, ir);

      const instance = new WebAssembly.Instance(
        new WebAssembly.Module(buf),
        importObject,
      ) as TypedWebAssemblyInstance<ModuleExports>;
      Object.assign(dynamic, instance.exports);

      const result = instance.exports.start();
      if (printEvalResult && fromSrc !== 0) {
        const writeClosure = getGlobal("write");
        const writeParams = runtimeInstance.exports.new_args(1);
        runtimeInstance.exports.set_args(writeParams, 0, result);
        runtimeInstance.exports.call_closure(writeClosure, writeParams);

        const newlineClosure = getGlobal("newline");
        const newlineParams = runtimeInstance.exports.new_args(0);
        runtimeInstance.exports.call_closure(newlineClosure, newlineParams);
      }
    },
    js_webschembly_log: (bufPtr, bufLen) => {
      const s = new TextDecoder().decode(
        new Uint8Array(runtimeInstance.exports.memory.buffer, bufPtr, bufLen),
      );
      logger.log(s);
    },
    js_write_buf: (fd, bufPtr, bufLen) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufLen,
      );
      writeBuf(fd, buf);
    },
  };

  const runtimeInstance = new WebAssembly.Instance(
    await loadRuntimeModule(),
    {
      env: runtimeImportObjects,
    } satisfies RuntimeImports,
  ) as TypedWebAssemblyInstance<RuntimeExports>;

  if (compilerConfig?.enableJit !== undefined) {
    runtimeInstance.exports.compiler_config_enable_jit(
      Number(compilerConfig.enableJit),
    );
  }
  if (compilerConfig?.enableJitOptimization !== undefined) {
    runtimeInstance.exports.compiler_config_enable_jit_optimization(
      Number(compilerConfig.enableJitOptimization),
    );
  }

  if (compilerConfig?.enableJitSmallBlockFusion !== undefined) {
    runtimeInstance.exports.compiler_config_enable_jit_block_fusion(
      Number(compilerConfig.enableJitSmallBlockFusion),
    );
  }

  if (compilerConfig?.enableJitLargeBlockFusion !== undefined) {
    runtimeInstance.exports.compiler_config_enable_jit_large_block_fusion(
      Number(compilerConfig.enableJitLargeBlockFusion),
    );
  }

  runtimeInstance.exports.init();

  const importObject: ModuleImports = {
    runtime: runtimeInstance.exports,
    dynamic,
  };

  const errorHandle = <A extends any[], R>(f: (...args: A) => R) => (...args: A): void => {
    try {
      f(...args);
    } catch (e) {
      if (
        e instanceof WebAssembly.Exception
        && e.is(runtimeInstance.exports.WEBSCHEMBLY_EXCEPTION)
      ) {
        if (exitWhenException) {
          exit(1);
        }
      } else {
        throw e;
      }
    }
  };

  function mallocString(s: string): [number, number] {
    const buf = new TextEncoder().encode(s);
    const bufPtr = runtimeInstance.exports.malloc(buf.length);
    new Uint8Array(runtimeInstance.exports.memory.buffer).set(buf, bufPtr);
    return [bufPtr, buf.length];
  }

  function getGlobal(name: string): SchemeValue {
    const [namePtr, nameLen] = mallocString(name);
    const id = runtimeInstance.exports.get_global_id(namePtr, nameLen);
    runtimeInstance.exports.free(namePtr);
    const global = dynamic[`global_${id}`];
    if (global === undefined || !(global instanceof WebAssembly.Global)) {
      throw new Error(`global not found: ${name}`);
    }
    return global.value;
  }

  return {
    loadStdlib: errorHandle(() => {
      runtimeInstance.exports.load_stdlib();
    }),
    loadSrc: errorHandle((srcBuf) => {
      const srcBufPtr = runtimeInstance.exports.malloc(srcBuf.length);
      new Uint8Array(runtimeInstance.exports.memory.buffer).set(
        srcBuf,
        srcBufPtr,
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
    mallocString,
    instance: runtimeInstance,
    getGlobal,
  };
}
