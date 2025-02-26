import { createRuntime } from "./runtime.js";

const runtime = createRuntime({
  runtimeName: "repl.scm",
  exitWhenException: false,
});

runtime.loadStdlib();

(async () => {
  process.stdout.write("=> ");
  for await (const chunk of process.stdin) {
    runtime.loadSrc(new Uint8Array(chunk));
    runtime.flushAll();
    process.stdout.write("=> ");
  }
})();
