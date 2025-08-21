# webschembly-compiler-cli

Command-line interface for the Webschembly compiler

## 使用例

```bash
$ cargo run -- a.scm
// a.wasmとa.1.wasmが生成される

$ cargo run -- --no-stdlib a.scm
// a.wasmのみが生成される

$ cargo run -- --ir a.scm
// a.irが生成される

$ cargo run -- --split-bb a.scm
// JITのために関数が分割されたwasmが生成される
```
