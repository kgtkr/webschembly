const fs = require("fs");
const path = require("path");

function getRuntime(srcName) {
  const logDir = process.env.LOG_DIR || null;
  const logBasename = path.basename(srcName) + "-" + Date.now();
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
  // TODO: 一旦大きめに確保、今後growする機能を追加
  const table = new WebAssembly.Table({ initial: 10000, element: "anyfunc" });

  const runtimeImportObjects = {
    instantiate: (bufPtr, bufSize) => {
      const buf = new Uint8Array(
        runtimeInstance.exports.memory.buffer,
        bufPtr,
        bufSize
      );
      if (logDir !== null) {
        fs.writeFileSync(
          path.join(logDir, logBasename + "-" + instantiateCount + ".wasm"),
          buf
        );
      }
      instantiateCount++;

      // TODO: free memory
      return new WebAssembly.Instance(
        new WebAssembly.Module(buf),
        importObject
      );
    },
    webschembly_log: (bufPtr, bufSize) => {
      const s = new TextDecoder().decode(
        new Uint8Array(runtimeInstance.exports.memory.buffer, bufPtr, bufSize)
      );
      if (logFile !== null) {
        fs.writeSync(logFile, s + "\n");
      }
    },
  };

  const runtimeInstance = new WebAssembly.Instance(
    new WebAssembly.Module(new Uint8Array(fs.readFileSync("bin/runtime.wasm"))),
    {
      env: runtimeImportObjects,
    }
  );
  runtimeInstance.exports.init();

  function valueToString(x) {
    const dataView = new DataView(runtimeInstance.exports.memory.buffer);

    const typeMask = ((1n << 4n) - 1n) << 48n;
    const valueMask = (1n << 48n) - 1n;

    const typeId = Number((x & typeMask) >> 48n);
    const value = Number(x & valueMask);

    switch (typeId) {
      case 1:
        return "()";
      case 2:
        return value === 0 ? "#f" : "#t";
      case 3:
        return value.toString();
      case 4:
        const car = dataView.getBigUint64(value, true);
        const cdr = dataView.getBigUint64(value + 8, true);

        return `(${valueToString(car)} . ${valueToString(cdr)})`;
      case 5:
        const length = dataView.getUint32(value, true);
        const string = new TextDecoder().decode(
          new Uint8Array(
            runtimeInstance.exports.memory.buffer,
            value + 4,
            length
          )
        );
        return `"${string}"`;
      case 6:
        return `<closure#${dataView.getUint32(value, true)})>`;
      case 7:
        return `<symbol#${value}>`;
      default:
        throw new Error(`unknown type: ${typeId}`);
    }
  }

  const importObject = {
    runtime: {
      ...runtimeInstance.exports,
      table,
      dump: (x) => {
        console.log(valueToString(x));
      },
    },
  };

  return runtimeInstance;
}

module.exports = { getRuntime };
