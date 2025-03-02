import * as fs from "fs";
import { createRuntime } from "./runtime.js";
import { createNodeRuntimeEnv } from "./node-runtime-env.js";

const srcName = process.argv[2];
if (!srcName) {
  console.error("Usage: run.js <src>");
  process.exit(1);
}
const runtime = createRuntime(
  createNodeRuntimeEnv({
    runtimeName: srcName,
  }),
  {}
);

const srcBuf = new Uint8Array(fs.readFileSync(srcName));

runtime.loadStdlib();
runtime.loadSrc(srcBuf);
runtime.cleanup();
