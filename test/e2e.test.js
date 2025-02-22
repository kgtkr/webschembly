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
      let stdout = "";
      let stderr = "";
      const runtime = createRuntime({
        runtimeName: filename,
        exit: (code) => {
          exitCode = code;
        },
        println: (s) => {
          stdout += s + "\n";
        },
        eprintln: (s) => {
          stderr += s + "\n";
        },
        runtimeBuf,
      });

      runtime.loadStdlib();
      runtime.loadSrc(srcBuf);
      expect({ exitCode, stdout, stderr }).toMatchSnapshot();
    });
  });
});
