# webschembly-compiler-cli

Command-line interface for the Webschembly compiler

## Usage Examples

```bash
$ cargo run -- a.scm
// Generates a.wasm and a.1.wasm

$ cargo run -- --no-stdlib a.scm
// Generates only a.wasm

$ cargo run -- --ir a.scm
// Generates a.ir

$ cargo run -- --split-bb a.scm
// Generates wasm with functions split for JIT
```
