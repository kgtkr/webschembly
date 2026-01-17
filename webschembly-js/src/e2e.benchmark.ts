import { Bench, type BenchOptions } from "tinybench";

import * as fs from "fs/promises";
import * as path from "path";
import { createNodeRuntimeEnv } from "./node-runtime-env";
import {
  type CompilerConfig,
  compilerConfigToString,
  createRuntime,
  type Runtime,
  type SchemeValue,
} from "./runtime";
import * as testUtils from "./test-utils";
import { createRequire } from "module";
const require = createRequire(import.meta.url);
const GUILE_HOOT_DIR = process.env.GUILE_HOOT_DIR;
const Hoot = require(GUILE_HOOT_DIR + "/reflect-js/reflect.js");

const filenames = (await testUtils.getAllFixtureFilenames()).filter((file) =>
  file.endsWith(".b.scm"),
);
console.log("Benchmarking files:", filenames.join(", "));
const compilerConfigs: CompilerConfig[] = [
  {},
  // { enableJitOptimization: false },
  { enableJit: false },
];

const runtimeModule = new WebAssembly.Module(
  await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]!),
);

// time[ms]経つ and iterations回という条件でベンチマークが終了する仕様になっている
// そのためsetupに時間が掛かるが本体は速いベンチマークだと数時間掛かってしまう
// その対策としてtime=0にしてiterations回で必ず終了するようにする
const benchOptions: BenchOptions = {
  time: 0,
  warmupTime: 0,
};
const bench = new Bench(
  process.env["BENCH_DEV"]
    ? {
        ...benchOptions,
        iterations: 10,
        warmupIterations: 5,
      }
    : benchOptions,
);

for (const filename of filenames) {
  for (const warmup of [false, true]) {
    for (const compilerConfig of compilerConfigs) {
      const srcBuf = await fs.readFile(
        path.join(testUtils.fixtureDir, filename),
      );

      let runtime: Runtime;

      if (warmup) {
        let runClosure: SchemeValue;
        let runArgs: SchemeValue;
        let afterWarmup = false;
        bench.add(
          `${filename},with warmup,${compilerConfigToString(compilerConfig)}`,
          () => {
            runtime.instance.exports.call_closure(runClosure, runArgs);
          },
          {
            beforeEach: async () => {
              afterWarmup = false;
              runtime = await createRuntime(
                await createNodeRuntimeEnv({
                  runtimeName: filename,
                  exit: () => {},
                  writeBuf: () => {},
                  loadRuntimeModule: async () => runtimeModule,
                  logger: {
                    log: () => {},
                    instantiate: () => {
                      if (afterWarmup) {
                        throw new Error(
                          "instantiate should not be called after warmup",
                        );
                      }
                    },
                  },
                }),
                {
                  compilerConfig,
                },
              );
              runtime.loadStdlib();
              runtime.loadSrc(srcBuf);
              runClosure = runtime.getGlobal("run");
              runArgs = runtime.instance.exports.new_args(0);
              // branch specializationのthresholdが20なので少し多めの30回実行する
              for (let i = 0; i < 30; i++) {
                runtime.instance.exports.call_closure(runClosure, runArgs);
              }
              afterWarmup = true;
            },
            afterEach: () => {
              runtime.cleanup();
            },
          },
        );
      } else {
        bench.add(
          `${filename},${compilerConfigToString(compilerConfig)}`,
          () => {
            runtime.loadSrc(srcBuf);
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
                },
              );
              runtime.loadStdlib();
            },
            afterEach: () => {
              runtime.cleanup();
            },
          },
        );
      }
    }
  }

  {
    const hootWasm = path.join(
      testUtils.fixtureDir,
      filename.replace(/\.scm$/, ".hoot.wasm"),
    );
    if (!(await fs.stat(hootWasm).catch(() => false))) {
      continue;
    }

    let runClosure: any;
    bench.add(
      `${filename}, hoot`,
      () => {
        runClosure.call();
      },
      {
        beforeEach: async () => {
          let [run] = await Hoot.Scheme.load_main(hootWasm, {
            reflect_wasm_dir: GUILE_HOOT_DIR + "/reflect-wasm",
          });
          runClosure = run;
        },
        afterEach: () => {
          // noop
        },
      },
    );
  }
}

let count = 0;
bench.addEventListener("cycle", (e) => {
  count++;
  console.log(`${count}/${bench.tasks.length}`);
});

await bench.run();
console.table(bench.table());

// benchmark.js互換形式で保存
const outputFile = await fs.open("benchmark.result", "w");
bench.tasks.forEach((task) => {
  const result = task.result!;
  outputFile.write(
    `${task.name} x ${result.throughput.mean.toFixed(
      2,
    )} ops/sec ±${result.latency.rme.toFixed(2)}% (${result.latency.samples.length} runs sampled)\n`,
  );
});
await outputFile.close();
