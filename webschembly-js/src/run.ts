import * as fs from "fs";
import { createRuntime } from "./runtime";
import { createNodeRuntimeEnv } from "./node-runtime-env";

const srcName = process.argv[2];
if (!srcName) {
  console.error("Usage: run <src>");
  process.exit(1);
}
const runtime = await createRuntime(
  await createNodeRuntimeEnv({
    runtimeName: srcName,
  }),
  {}
);

const srcBuf = new Uint8Array(fs.readFileSync(srcName));

runtime.loadStdlib();
runtime.loadSrc(srcBuf);
runtime.cleanup();
