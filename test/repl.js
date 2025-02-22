import { createRuntime } from "./runtime.js";
import * as fs from "fs";

const runtime = createRuntime({ runtimeName: "repl.scm" });

runtime.loadStdlib();

(async () => {
  for await (const chunk of process.stdin) {
    runtime.loadSrc(new Uint8Array(chunk));
  }
})();
