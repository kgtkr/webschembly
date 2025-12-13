import * as fs from "fs/promises";
import * as fsLegacy from "fs";
import { beforeAll, describe, expect, test } from "vitest";
import * as path from "path";
import {
  compilerConfigToString,
  createRuntime,
  type CompilerConfig,
} from "./runtime";
import { createNodeRuntimeEnv } from "./node-runtime-env";
import * as testUtils from "./test-utils";

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

const snapshotDir = "e2e_snapshots";

const compilerConfigs: CompilerConfig[] = [
  {},
  { enableJitOptimization: false },
  { enableJit: false },
];

describe("E2E test", async () => {
  let runtimeModule: WebAssembly.Module;
  const filenames = await testUtils.getAllFixtureFilenames();
  beforeAll(async () => {
    runtimeModule = new WebAssembly.Module(
      await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]!)
    );
  });

  describe.each(
    compilerConfigs.map((compilerConfig) => [
      compilerConfigToString(compilerConfig),
      compilerConfig,
    ])
  )("%s", (_, compilerConfig) => {
    describe.each(filenames)("%s", (filename) => {
      let srcBuf: Buffer;
      beforeAll(async () => {
        srcBuf = await fs.readFile(path.join(testUtils.fixtureDir, filename));
      });

      test(
        "snapshot test",
        async () => {
          let exitCode = 0;
          const stdoutBufs: Uint8Array[] = [];
          const stderrBufs: Uint8Array[] = [];
          const runtime = await createRuntime(
            await createNodeRuntimeEnv({
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
              loadRuntimeModule: async () => runtimeModule,
            }),
            {
              compilerConfig,
            }
          );

          runtime.loadStdlib();
          runtime.loadSrc(srcBuf);
          runtime.cleanup();

          const stdout = new TextDecoder().decode(concatBufs(stdoutBufs));
          const stderr = new TextDecoder().decode(concatBufs(stderrBufs));

          await expect(exitCode).toMatchFileSnapshot(
            `${snapshotDir}/${filename}-exitCode`
          );
          await expect(stdout).toMatchFileSnapshot(
            `${snapshotDir}/${filename}-stdout`
          );
          await expect(stderr).toMatchFileSnapshot(
            `${snapshotDir}/${filename}-stderr`
          );
        },
        60 * 1000
      );
    });
  });
});
