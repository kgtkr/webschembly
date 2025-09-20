import Benchmark from "benchmark";

import * as fs from "fs/promises";
import * as path from "path";
import {
  compilerConfigToString,
  createRuntime,
  type CompilerConfig,
} from "./runtime";
import { createNodeRuntimeEnv } from "./node-runtime-env";

const sourceDir = "fixtures";
const filenames = (await fs.readdir(sourceDir)).filter((file) =>
  file.endsWith(".b.scm")
);

const compilerConfigs: CompilerConfig[] = [
  { enableJit: true },
  { enableJit: false },
];

const runtimeModule = new WebAssembly.Module(
  await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]!)
);

const suite = new Benchmark.Suite();

for (const compilerConfig of compilerConfigs) {
  for (const filename of filenames) {
    const srcBuf = await fs.readFile(path.join(sourceDir, filename));
    const runtime = await createRuntime(
      await createNodeRuntimeEnv({
        runtimeName: filename,
        exit: () => {},
        writeBuf: () => {},
        loadRuntimeModule: async () => runtimeModule,
      }),
      {
        compilerConfig,
      }
    );

    suite.add(`${filename},${compilerConfigToString(compilerConfig)}`, () => {
      runtime.loadStdlib();
      runtime.loadSrc(srcBuf);
      runtime.cleanup();
    });
  }
}

suite
  .on("cycle", (event: any) => {
    console.log(String(event.target));
  })
  .run();
