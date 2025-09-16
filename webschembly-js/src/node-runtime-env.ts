import * as fs from "fs/promises";
import * as path from "path";
import { type RuntimeLogger, type RuntimeEnv } from "./runtime";

export async function createLogger({
  logDir = process.env.LOG_DIR || null,
  runtimeName = "untitled",
}): Promise<RuntimeLogger> {
  const logBasename = path.basename(runtimeName) + "-" + Date.now();
  let logFile = null;
  if (logDir !== null) {
    try {
      await fs.mkdir(logDir);
    } catch (e: any) {
      if (e.code !== "EEXIST") {
        throw e;
      }
    }

    logFile = await fs.open(path.join(logDir, logBasename + ".log"), "a");
  }
  let instantiateCount = 0;

  return {
    instantiate: (buf, ir) => {
      if (logDir !== null) {
        void fs.writeFile(
          path.join(logDir, logBasename + "-" + instantiateCount + ".wasm"),
          buf
        );
        if (ir !== null) {
          void fs.writeFile(
            path.join(logDir, logBasename + "-" + instantiateCount + ".ir"),
            ir
          );
        }
      }
      instantiateCount++;
    },
    log: (s) => {
      if (logFile !== null) {
        void fs.writeFile(logFile, s + "\n", { flush: true });
        // TODO: panicするとflushされないことがある
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
      await fs.readFile(process.env["WEBSCHEMBLY_RUNTIME"]!)
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
