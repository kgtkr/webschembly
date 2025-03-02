import playgroundWorker from "./playground.worker.js?worker";

let runtimeModule;
fetch(import.meta.env.BASE_URL + "wasm/webschembly_runtime.wasm").then((res) =>
  res.arrayBuffer().then((buf) => {
    runtimeModule = new WebAssembly.Module(buf);
  })
);

document.addEventListener("DOMContentLoaded", () => {
  const src = document.getElementById("src");
  const runButton = document.getElementById("run");
  const stdout = document.getElementById("stdout");
  const stderr = document.getElementById("stderr");
  const exitCode = document.getElementById("exit-code");

  runButton.addEventListener("click", () => {
    const worker = new playgroundWorker();
    worker.postMessage({ src: src.value, runtimeModule });

    worker.addEventListener("message", (event) => {
      const { exitCode: code, stdout: out, stderr: err } = event.data;
      exitCode.textContent = code;
      stdout.textContent = out;
      stderr.textContent = err;
    });
  });
});
