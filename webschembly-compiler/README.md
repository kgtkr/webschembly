# webschembly-compiler

Scheme to WebAssembly compiler library

## 主要モジュール
* `token`: トークンの定義
* `lexer`: 字句解析器
* `ast`: 抽象構文木の定義
* `ir`: 中間表現の定義
* `ir_generator`: IR生成器。IRの変形なども含む
* `wasm_generator`: WebAssembly生成器
* `stdlib`: 標準ライブラリの定義

## テスト

```bash
cargo test
```
