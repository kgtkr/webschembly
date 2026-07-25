# WebAssembly ランタイムと JIT 開発ルール

## WebAssembly の機能
プロジェクトでは高度な Wasm 拡張機能を有効化してビルドしている (`binaryen-args.txt`):
- GC (`--enable-gc`)
- Reference Types (`--enable-reference-types`)
- Multi Memory (`--enable-multimemory`)
- Bulk Memory (`--enable-bulk-memory`)
- Multi Value (`--enable-multivalue`)
- Exception Handling (`--enable-exception-handling`)
- SIMD (`--enable-simd`)
- Tail Call (`--enable-tail-call`)

## Scheme 値の表現 (Wasm GC)
Scheme の値はすべて Wasm GC の struct/array で表現し、JS 側でのガベージコレクションは行わない (Wasm ネイティブ GC に依存)。
定義は `webschembly-runtime/lib.wat` に記述。

- `$StringBuf`: Shared フラグ付きの Copy-on-Write バッファ
- `$String`: 共有バッファへの参照、オフセット、長さを持つ
- `$Cons`: `(mut eqref)` の car/cdr フィールド
- `$Closure`: モジュールID、関数ID、環境インデックス、エントリポイントテーブルなどを持つ
- `$MutFuncRef`: JIT で動的パッチを行うための `(mut funcref)` コンテナ

## ランタイム構成 (WAT + Rust)
- WAT 側 (`lib.wat`): Wasm GC 型定義、基本プリミティブ (`string_copy`, `args_to_list` など)
- Rust 側 (`webschembly-runtime-rust`): シンボル管理、カスタムアロケータ (`malloc`/`free`)、入出力バッファ、JIT呼び出し元
- 結合: `wasm-as` で WAT をコンパイルし、`wasm-merge` (Binaryen) で Rust 側 wasm と結合し `webschembly_runtime.wasm` を生成する

## メモリ管理
- リニアメモリは Rust のヒープ領域 (`malloc`/`free`) および、Wasm GC ↔ リニアメモリ間のデータ転送(文字列変換など)にのみ使用する
- リニアメモリから GC メモリへの転送はバイトごとのループコピー処理を WAT 内で実行

## JIT コンパイルアーキテクチャ
- 実行時の段階的コンパイル (Tiered Compilation)
- **関数レベルJIT (`instantiate_func`)**: 実行されるクロージャの環境に基づいて特化した Wasm 関数を生成する
- **ベーシックブロックレベルJIT (`instantiate_bb`)**: ブロック単位でさらに細かくコンパイル
- **プロファイル駆動最適化 (`increment_branch_counter`)**:
  - ブランチの実行回数をカウント
  - 閾値を超えた場合に再コンパイルをトリガーし、最適化されたパスを生成
- **動的パッチ**: `MutFuncRef` を更新することで、実行中の関数ポインタを JIT された最適化コードへ書き換える
- **Block Fusion**: 複数のベーシックブロックを結合して Wasm を生成する (SmallFusion / LargeFusion)
