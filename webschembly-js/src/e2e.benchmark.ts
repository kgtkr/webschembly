import { Bench } from "tinybench";

import * as fs from "fs/promises";
import * as path from "path";
import {
  compilerConfigToString,
  createRuntime,
  type CompilerConfig,
  type Runtime,
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

const bench = new Bench();

for (const compilerConfig of compilerConfigs) {
  for (const filename of filenames) {
    const srcBuf = await fs.readFile(path.join(sourceDir, filename));
    let runtime: Runtime;

    bench.add(
      `${filename},${compilerConfigToString(compilerConfig)}`,
      () => {
        runtime.loadStdlib();
        runtime.loadSrc(srcBuf);
        runtime.cleanup();
      },
      {
        beforeEach: async () => {
          runtime = await createRuntime(
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
        },
      }
    );
  }
}

await bench.run();
console.table(bench.table());

// benchmark.js互換形式で保存
const outputFile = await fs.open("benchmark.result", "w");
bench.tasks.forEach((task) => {
  const result = task.result!;
  outputFile.write(
    `${task.name} x ${result.throughput.mean.toFixed(
      2
    )} ops/sec ±${result.latency.rme.toFixed(2)}% (${
      result.latency.samples.length
    } runs sampled)\n`
  );
});
await outputFile.close();
