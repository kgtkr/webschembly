import { createRuntime } from "webschembly-js/runtime.js";

self.addEventListener("message", (event) => {
  const { src, runtimeModule } = event.data;

  const srcBuf = new TextEncoder().encode(src);
  let exitCode = 0;
  const stdoutBufs = [];
  const stderrBufs = [];
  const runtime = createRuntime(
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
      runtimeModule,
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
    {}
  );

  runtime.loadStdlib();
  runtime.loadSrc(srcBuf);
  runtime.cleanup();

  const stdout = new TextDecoder().decode(concatBufs(stdoutBufs));
  const stderr = new TextDecoder().decode(concatBufs(stderrBufs));

  self.postMessage({ exitCode, stdout, stderr });
});

function concatBufs(bufs) {
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
