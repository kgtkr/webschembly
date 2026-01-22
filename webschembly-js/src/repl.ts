import { createNodeRuntimeEnv } from "./node-runtime-env.js";
import { createRuntime } from "./runtime.js";

const runtime = await createRuntime(
  await createNodeRuntimeEnv({
    runtimeName: "repl.scm",
  }),
  {
    exitWhenException: false,
    printEvalResult: true,
  },
);

process.stdout.write("=> <eval stdlib>\n");
runtime.loadStdlib();

(async () => {
  process.stdout.write("=> ");
  for await (const chunk of process.stdin) {
    runtime.loadSrc(new Uint8Array(chunk));
    runtime.flushAll();
    process.stdout.write("=> ");
  }
})();
