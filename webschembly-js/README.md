# webschembly-js

JavaScript bindings for Webschembly


## 使用方法
環境変数 `LOG_DIR` を設定することで、ログファイルの出力先を指定できます。
```bash
$ make repl
// REPL起動
$ make test
// E2Eテスト実行
$ make run SRC=./a.scm
// JITモードで実行
$ make run-aot SRC=./a.scm
// AOTモードで実行
```
