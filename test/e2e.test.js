import * as fs from "fs/promises";
import * as fsLegacy from "fs";
import { beforeAll, describe, expect, test } from "vitest";
import * as path from "path";
import { createRuntime } from "./runtime.js";

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
      let stderr = "";
      const stdoutBufs = [];
      const runtime = createRuntime({
        runtimeName: filename,
        exit: (code) => {
          exitCode = code;
        },
        eprintln: (s) => {
          stderr += s + "\n";
        },
        writeBuf: (buf) => {
          stdoutBufs.push(buf);
        },
        runtimeBuf,
      });

      runtime.loadStdlib();
      runtime.loadSrc(srcBuf);
      const stdoutBufLen = stdoutBufs
        .map((buf) => buf.length)
        .reduce((a, b) => a + b, 0);
      const stdoutBuf = new Uint8Array(stdoutBufLen);
      let offset = 0;
      for (const buf of stdoutBufs) {
        stdoutBuf.set(buf, offset);
        offset += buf.length;
      }
      const stdout = new TextDecoder().decode(stdoutBuf);

      expect({ exitCode, stdout, stderr }).toMatchSnapshot();
    });
  });
});
