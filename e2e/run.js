const fs = require("fs");

const runtimeImportObjects = {
  instantiate: (bufPtr, bufSize) => {
    // TODO: free memory
    return new WebAssembly.Instance(
      new WebAssembly.Module(
        new Uint8Array(runtimeInstance.exports.memory.buffer, bufPtr, bufSize)
      ),
      importObject
    );
  },
  webschembly_log: (bufPtr, bufSize) => {
    console.log(
      new TextDecoder().decode(
        new Uint8Array(runtimeInstance.exports.memory.buffer, bufPtr, bufSize)
      )
    );
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
        new Uint8Array(runtimeInstance.exports.memory.buffer, value + 4, length)
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
    dump: (x) => {
      console.log(valueToString(x));
    },
  },
};

const srcBuf = new Uint8Array(fs.readFileSync(process.argv[2]));
const srcBufPtr = runtimeInstance.exports.malloc(srcBuf.length);
new Uint8Array(runtimeInstance.exports.memory.buffer).set(srcBuf, srcBufPtr);
runtimeInstance.exports.run(srcBufPtr, srcBuf.length);
