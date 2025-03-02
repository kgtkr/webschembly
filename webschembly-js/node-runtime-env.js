import * as fs from "fs";
import * as path from "path";

export function createLogger({
  logDir = process.env.LOG_DIR || null,
  runtimeName = "untitled",
}) {
  const logBasename = path.basename(runtimeName) + "-" + Date.now();
  let logFile = null;
  if (logDir !== null) {
    try {
      fs.mkdirSync(logDir);
    } catch (e) {
      if (e.code !== "EEXIST") {
        throw e;
      }
    }

    logFile = fs.openSync(path.join(logDir, logBasename + ".log"), "a");
  }
  let instantiateCount = 0;

  return {
    instantiate: (buf) => {
      if (logDir !== null) {
        fs.writeFileSync(
          path.join(logDir, logBasename + "-" + instantiateCount + ".wasm"),
          buf
        );
      }
      instantiateCount++;
    },
    log: (s) => {
      if (logFile !== null) {
        fs.writeSync(logFile, s + "\n");
      }
    },
  };
}

export function createNodeRuntimeEnv({
  runtimeName = "untitled",
  exit = process.exit,
  logger = createLogger({ runtimeName }),
  runtimeModule = new WebAssembly.Module(
    fs.readFileSync(process.env["WEBSCHEMBLY_RUNTIME"])
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
}) {
  return {
    runtimeName,
    exit,
    logger,
    runtimeModule,
    writeBuf,
  };
}
