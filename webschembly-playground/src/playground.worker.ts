import { createRuntime } from "webschembly-js/runtime";

self.addEventListener("message", async (event) => {
  const {
    src,
    runtimeModule,
  }: {
    src: string;
    runtimeModule: WebAssembly.Module;
  } = event.data;

  const srcBuf = new TextEncoder().encode(src);
  let exitCode = 0;
  const stdoutBufs: Uint8Array[] = [];
  const stderrBufs: Uint8Array[] = [];
  const runtime = await createRuntime(
    {
      exit: (code) => {
        exitCode = code;
      },
      logger: {
        log: (s) => {
          console.log("compiler log:", s);
        },
        instantiate: (buf) => {
          console.log("instantiate:", buf);
        },
      },
      loadRuntimeModule: async () => runtimeModule,
      writeBuf: (fd, buf) => {
        switch (fd) {
          case 1:
            stdoutBufs.push(new Uint8Array(buf));
            break;
          case 2:
            stderrBufs.push(new Uint8Array(buf));
            break;
          default:
            throw new Error(`Unsupported file descriptor: ${fd}`);
        }
      },
    },
    {},
  );

  runtime.loadStdlib();
  runtime.loadSrc(srcBuf);
  runtime.cleanup();

  const stdout = new TextDecoder().decode(concatBufs(stdoutBufs));
  const stderr = new TextDecoder().decode(concatBufs(stderrBufs));

  self.postMessage({ exitCode, stdout, stderr });
});

function concatBufs(bufs: Uint8Array[]) {
  const bufLen = bufs.map((buf) => buf.length).reduce((a, b) => a + b, 0);
  const resultBuf = new Uint8Array(bufLen);
  let offset = 0;
  for (const buf of bufs) {
    resultBuf.set(buf, offset);
    offset += buf.length;
  }
  return resultBuf;
}

export default {};
