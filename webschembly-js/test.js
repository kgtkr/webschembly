import * as fs from "fs/promises";
import { createRequire } from "module";
const require = createRequire(import.meta.url);
const GUILE_HOOT_DIR = process.env.GUILE_HOOT_DIR;
const { Scheme } = require(GUILE_HOOT_DIR + "/reflect-js/reflect.js");

async function runWasm() {
  let [run] = await Scheme.load_main("./fixtures/div2.b.hoot.wasm", {
    reflect_wasm_dir: GUILE_HOOT_DIR + "/reflect-wasm",
  });
  console.log(run.call());
}

runWasm();
