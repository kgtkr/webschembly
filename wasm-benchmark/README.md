wasmの実験

```
$ wasm-as standard.wat && node main.js standard.wasm
Total time: 102.50 ms
Result: 1229

$ wasm-as --enable-tail-call bb.wat && node main.js bb.wasm
Total time: 172.34 ms
Result: 1229

$ wasm-as --enable-tail-call --enable-gc --enable-reference-types bb_struct.wat && node main.js bb_struct.wasm
Total time: 170.24 ms
Result: 1229
```
