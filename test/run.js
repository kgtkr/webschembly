import * as fs from "fs";
import { createRuntime } from "./runtime.js";

const srcName = process.argv[2];
const runtime = createRuntime({ runtimeName: srcName });

const srcBuf = new Uint8Array(fs.readFileSync(srcName));

runtime.loadStdlib();
runtime.loadSrc(srcBuf);
