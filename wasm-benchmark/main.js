const fs = require("fs");
const { performance } = require("perf_hooks");

async function main() {
  const wasmPath = process.argv[2];

  const wasmBuffer = fs.readFileSync(wasmPath);
  const { instance } = await WebAssembly.instantiate(wasmBuffer);
  const prime_count = instance.exports.prime_count;

  const t0 = performance.now();

  const result = prime_count(10000);

  const t1 = performance.now();

  console.log(`Total time: ${(t1 - t0).toFixed(2)} ms`);
  console.log(`Result: ${result}`);
}

main();
