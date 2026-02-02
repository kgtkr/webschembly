import { Bench, type BenchOptions } from "tinybench";

import * as fs from "fs/promises";
import { createRequire } from "module";
import * as path from "path";
import { createNodeRuntimeEnv } from "./node-runtime-env.js";
import {
  type CompilerConfig,
  compilerConfigToString,
  createRuntime,
  type Runtime,
  type SchemeValue,
} from "./runtime.js";
import * as testUtils from "./test-utils.js";
const require = createRequire(import.meta.url);
const kindShardPrefix = process.env.WEBSCHEMBLY_KIND_SHARD_PREFIX || "";
const GUILE_HOOT_DIR = process.env.GUILE_HOOT_DIR;
const Hoot = GUILE_HOOT_DIR && require(GUILE_HOOT_DIR + "/reflect-js/reflect.js");

type WarmupKind = "none" | "static" | "dynamic";
const filenames = (await testUtils.getAllFixtureFilenames()).filter((file) => file.endsWith(".b.scm"));
console.log("Benchmarking files:", filenames.join(", "));
const compilerConfigs: CompilerConfig[] = [
  // { enableJitOptimization: false },
  { enableJit: false },
  { enableJitSmallBlockFusion: false, enableJitLargeBlockFusion: false },
  { enableJitSmallBlockFusion: false, enableJitLargeBlockFusion: true },
  { enableJitSmallBlockFusion: true, enableJitLargeBlockFusion: false },
];

const noLiftoff = process.env["NO_LIFTOFF"] === "1";
const noLiftoffPrefix = noLiftoff ? "no_liftoff," : "";

const runtimeModule = new WebAssembly.Module(
  (await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]!)) as any,
);

type BenchmarkKind =
  | {
    type: "webschembly";
    compilerConfig: CompilerConfig;
    warmup: WarmupKind;
  }
  | {
    type: "hoot";
  };

function getAllBenchmarkKinds(): BenchmarkKind[] {
  const kinds: BenchmarkKind[] = [];
  for (const compilerConfig of compilerConfigs) {
    for (const warmup of ["none", "static", "dynamic"] satisfies WarmupKind[]) {
      // JITが無効の時dynamic warmupとstatic warmupは同じなので除外
      if (warmup === "static" && compilerConfig.enableJit === false) {
        continue;
      }
      // noLiftoffのときはdynamicのみ含める
      if (noLiftoff && warmup !== "dynamic") {
        continue;
      }
      kinds.push({
        type: "webschembly",
        compilerConfig,
        warmup,
      });
    }
  }
  if (Hoot) {
    kinds.push({ type: "hoot" });
  }
  return kinds;
}

function benchmarkKindToString(kind: BenchmarkKind): string {
  if (kind.type === "webschembly") {
    if (kind.warmup === "none") {
      return compilerConfigToString(kind.compilerConfig);
    } else {
      return `with ${kind.warmup === "dynamic" ? "dynamic " : ""}warmup,${
        compilerConfigToString(
          kind.compilerConfig,
        )
      }`;
    }
  } else {
    return "hoot";
  }
}

const benchmarkKinds = getAllBenchmarkKinds();

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
      iterations: 1,
      warmupIterations: 0,
    }
    : benchOptions,
);

for (const filename of filenames) {
  for (const benchmarkKind of benchmarkKinds) {
    const benchmarkKindName = benchmarkKindToString(benchmarkKind);
    if (!testUtils.isShaPrefix(benchmarkKindName, kindShardPrefix)) {
      continue;
    }
    const benchmarkName = `${filename},${noLiftoffPrefix}${benchmarkKindName}`;
    if (benchmarkKind.type === "webschembly") {
      const { compilerConfig, warmup } = benchmarkKind;
      const srcBuf = await fs.readFile(
        path.join(testUtils.fixtureDir, filename),
      );

      let runtime: Runtime;

      if (warmup !== "none") {
        let runClosure: SchemeValue;
        let runArgs: SchemeValue;
        let afterWarmup = false;
        bench.add(
          benchmarkName,
          () => {
            runtime.instance.exports.call_closure(runClosure, runArgs);
          },
          {
            beforeEach: async () => {
              afterWarmup = false;
              let i = 0;
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
                      } else if (warmup === "dynamic") {
                        i = 0;
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
              const argValue = runtime.getGlobal("arg");
              runArgs = runtime.instance.exports.new_args(1);
              runtime.instance.exports.set_args(runArgs, 0, argValue);
              // branch specializationのthresholdが20なので少し多めの30回実行する
              while (i < 30) {
                runtime.instance.exports.call_closure(runClosure, runArgs);
                i++;
              }
              afterWarmup = true;
              globalThis.gc!();
            },
            afterEach: () => {
              runtime.cleanup();
            },
          },
        );
      } else {
        bench.add(
          benchmarkName,
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
              globalThis.gc!();
            },
            afterEach: () => {
              runtime.cleanup();
            },
          },
        );
      }
    } else {
      const hootWasm = path.join(
        testUtils.fixtureDir,
        filename.replace(/\.scm$/, ".hoot.wasm"),
      );
      if (await fs.stat(hootWasm).catch(() => false)) {
        let runClosure: any;
        let argValue: any;
        const originalStdoutWrite = process.stdout.write;
        const originalStderrWrite = process.stderr.write;

        bench.add(
          benchmarkName,
          () => {
            runClosure.call(argValue);
          },
          {
            beforeEach: async () => {
              process.stdout.write = () => true;
              process.stderr.write = () => true;

              [runClosure, argValue] = await Hoot.Scheme.load_main(hootWasm, {
                reflect_wasm_dir: GUILE_HOOT_DIR + "/reflect-wasm",
              });
              for (let i = 0; i < 30; i++) {
                runClosure.call(argValue);
              }
              globalThis.gc!();
            },
            afterEach: () => {
              process.stdout.write = originalStdoutWrite;
              process.stderr.write = originalStderrWrite;
            },
          },
        );
      }
    }
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
    `${task.name} x ${
      result.throughput.mean.toFixed(
        2,
      )
    } ops/sec ±${result.latency.rme.toFixed(2)}% (${result.latency.samples.length} runs sampled)\n`,
  );
});
await outputFile.close();

// json形式で生データを保存
await fs.writeFile(
  "benchmark.result.json",
  JSON.stringify(
    bench.tasks.map((task) => ({
      name: task.name,
      samples: task.result!.latency.samples,
    })),
  ),
);
