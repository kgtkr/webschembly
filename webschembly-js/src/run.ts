import * as fs from "fs";
import { createNodeRuntimeEnv } from "./node-runtime-env";
import { createRuntime } from "./runtime";

const srcName = process.argv[2];
if (!srcName) {
  console.error("Usage: run <src>");
  process.exit(1);
}
const runtime = await createRuntime(
  await createNodeRuntimeEnv({
    runtimeName: srcName,
  }),
  {},
);

const srcBuf = new Uint8Array(fs.readFileSync(srcName));

runtime.loadStdlib();
runtime.loadSrc(srcBuf);
runtime.cleanup();
