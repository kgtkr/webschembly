# webschembly-js

JavaScript bindings for Webschembly


## Usage
You can specify the log file output directory by setting the `LOG_DIR` environment variable.
```bash
$ make repl
// Start REPL
$ make test
// Run E2E tests
$ make run SRC=./a.scm
// Run in JIT mode
$ make run-aot SRC=./a.scm
// Run in AOT mode
```
