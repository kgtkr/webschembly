import * as fsSync from "fs";
import * as fs from "fs/promises";

import * as path from "path";
import { type RuntimeEnv, type RuntimeLogger } from "./runtime";

export async function createLogger({
  logDir = process.env.LOG_DIR || null,
  logStdout = process.env.LOG_STDOUT === "1",
  runtimeName = "untitled",
}): Promise<RuntimeLogger> {
  const logBasename = Date.now() + "-" + path.basename(runtimeName);
  let logFile = null;
  if (logDir !== null) {
    try {
      fsSync.mkdirSync(logDir);
    } catch (e: any) {
      if (e.code !== "EEXIST") {
        throw e;
      }
    }

    logFile = fsSync.openSync(path.join(logDir, logBasename + ".log"), "a");
  }
  let instantiateCount = 0;

  return {
    instantiate: (buf, ir) => {
      if (logDir !== null) {
        fsSync.writeFileSync(
          path.join(logDir, logBasename + "-" + instantiateCount + ".wasm"),
          buf,
        );
        if (ir !== null) {
          fsSync.writeFileSync(
            path.join(logDir, logBasename + "-" + instantiateCount + ".ir"),
            ir,
          );
        }
      }
      if (logStdout) {
        console.log(
          `called instantiate: id:${instantiateCount}, buf_size:${buf.length}`,
        );
      }
      instantiateCount++;
    },
    log: (s) => {
      if (logFile !== null) {
        fsSync.writeFileSync(logFile, s + "\n", { flush: true });
      }
      if (logStdout) {
        console.log(s);
      }
    },
  };
}

export async function createNodeRuntimeEnv({
  runtimeName = "untitled",
  exit = process.exit,
  logger,
  loadRuntimeModule = async () =>
    new WebAssembly.Module(
      await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]!),
    ),
  writeBuf = (fd, buf) => {
    switch (fd) {
      case 1:
        process.stdout.write(buf);
        break;
      case 2:
        process.stderr.write(buf);
        break;
      default:
        throw new Error(`Unsupported file descriptor: ${fd}`);
    }
  },
}: Partial<RuntimeEnv & { runtimeName: string }>): Promise<RuntimeEnv> {
  if (!logger) {
    logger = await createLogger({ runtimeName });
  }

  return {
    exit,
    logger,
    loadRuntimeModule,
    writeBuf,
  };
}
