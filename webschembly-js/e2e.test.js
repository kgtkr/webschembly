import * as fs from "fs/promises";
import * as fsLegacy from "fs";
import { beforeAll, describe, expect, test } from "vitest";
import * as path from "path";
import { createRuntime } from "./runtime.js";
import { createNodeRuntimeEnv } from "./node-runtime-env.js";

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

const sourceDir = "src";
const filenames = fsLegacy
  .readdirSync(sourceDir)
  .filter((file) => file.endsWith(".scm"));

describe("E2E test", () => {
  let runtimeBuf;
  beforeAll(async () => {
    runtimeBuf = await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]);
  });

  describe.each(filenames)("%s", (filename) => {
    let srcBuf;
    beforeAll(async () => {
      srcBuf = await fs.readFile(path.join(sourceDir, filename));
    });

    test("snapshot test", async () => {
      let exitCode = 0;
      const stdoutBufs = [];
      const stderrBufs = [];
      const runtime = createRuntime(
        createNodeRuntimeEnv({
          runtimeName: filename,
          exit: (code) => {
            exitCode = code;
          },
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
          runtimeBuf,
        }),
        {}
      );

      runtime.loadStdlib();
      runtime.loadSrc(srcBuf);
      runtime.cleanup();

      const stdout = new TextDecoder().decode(concatBufs(stdoutBufs));
      const stderr = new TextDecoder().decode(concatBufs(stderrBufs));

      expect({ exitCode, stdout, stderr }).toMatchSnapshot();
    });
  });
});
