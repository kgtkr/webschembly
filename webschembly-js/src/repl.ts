import { createNodeRuntimeEnv } from "./node-runtime-env";
import { createRuntime } from "./runtime";

const runtime = createRuntime(
  createNodeRuntimeEnv({
    runtimeName: "repl.scm",
  }),
  {
    exitWhenException: false,
    printEvalResult: true,
  }
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
