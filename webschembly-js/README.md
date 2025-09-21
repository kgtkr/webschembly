# webschembly-js

JavaScript bindings for Webschembly


## Usage
```bash
$ just repl
=> (+ 1 2)
$ just test
$ just benchmark
$ just run ./fixtures/add.scm
$ just LOG=1 run ./fixtures/add.scm # output log: /log/
$ just run-aot ./fixtures/add.wasm
```