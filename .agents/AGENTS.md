# Webschembly — Agent Rules

## プロジェクト概要

Webschembly は **Scheme を WebAssembly にJITコンパイルする処理系**。ブラウザ上で動作する Playground と Node.js CLI を提供する。

### コンポーネント構成

| ディレクトリ | 言語 | 役割 |
|---|---|---|
| `webschembly-compiler` | Rust | Scheme → Wasm コンパイラライブラリ (パーサ、AST、IR、Wasm生成) |
| `webschembly-compiler-cli` | Rust | コンパイラの CLI (デバッグ用) |
| `webschembly-compiler-crates/*` | Rust | コンパイラの補助クレート (ast, ir, error, sexpr, locate, vec-map, ast-generator) |
| `webschembly-runtime-rust` | Rust → Wasm | ランタイムの Rust 実装部分 (シンボル管理、I/O、JIT橋渡し) |
| `webschembly-runtime` | WAT + Nix | Wasm GC ランタイム (値の型定義、プリミティブ操作)。WAT + Rust Wasm を `wasm-merge` で結合 |
| `webschembly-js` | TypeScript | JS-Wasm ブリッジ、CLI ランナー、REPL、E2Eテスト・ベンチマーク |
| `webschembly-playground` | TypeScript/React | Web Playground (Vite, React 19, React Flow) |

### コンパイルパイプライン

```
Scheme Source
  → Lexer (nom) → Tokens
  → S-Expr Parser (nom) → SExpr
  → AST Generator (5段階: Parsed → Desugared → Defined → TailCall → Used)
  → IR Generator → SSA-based IR (Module)
  → IR Processor (最適化 → desugar → Phi除去 → レジスタ割り当て)
  → Wasm Generator (Relooper) → WebAssembly binary
```

### JIT コンパイル

- 関数単位 (`instantiate_func`) およびベーシックブロック単位 (`instantiate_bb`) の段階的JITコンパイル
- ブランチカウンタによるプロファイル駆動最適化 (`increment_branch_counter`)
- `MutFuncRef` による実行中の関数ポインタ書き換え
- Block Fusion (Small/Large) によるブロック結合最適化
- Closure 環境の型特殊化

## 言語とランタイム

- **Rust**: nightly-2025-10-09, edition 2024
  - 使用 nightly features: `string_deref_patterns`, `box_patterns`, `trait_alias`, `never_type`, `coroutines`, `if_let_guard`, `impl_trait_in_assoc_type`
- **TypeScript**: ES2020 target, ESNext modules, strict mode, `verbatimModuleSyntax: true`
- **WebAssembly**: GC, reference types, multi-memory, bulk-memory, multi-value, exception-handling, SIMD, tail-call
- **Nix**: flake-parts ベースのビルドオーケストレーション

## ビルドシステム

- **Rust ビルド**: Cargo workspace (ルート `Cargo.toml`) + Nix (`cargo2nix-ifd`)
- **JS ビルド**: npm workspaces (`webschembly-js`, `webschembly-playground`)
- **ランタイムビルド**: `wasm-as` (WAT→Wasm) → `wasm-merge` (WAT Wasm + Rust Wasm) → Binaryen 最適化
- **Playground ビルド**: Vite + React
- **全体オーケストレーション**: Nix flake (`flake.nix`, `rust.nix`, `js.nix`, `schemat.nix`)
- **開発環境**: `direnv` + Nix dev shell (`use flake`)、Makefile / Justfile が各サブプロジェクトにある

## フォーマッタ・リンター

- **treefmt** による統一フォーマット (CI では `treefmt --ci`)
  - Nix: `nixpkgs-fmt`
  - Rust: `rustfmt` (edition 2024)
  - Scheme/WAT: `schemat`
  - JS/TS/JSON/TOML/MD/CSS/HTML/YAML: `dprint`
- **Clippy**: CI で `cargo clippy` を実行。autofix ワークフローで自動修正

## テスト

- **Rust 単体テスト**: `cargo test` (lexer スナップショットテスト with `insta`、レジスタ割り当てテスト等)
- **E2E テスト**: `webschembly-js` の `vitest` によるスナップショットテスト
  - 5つのコンパイラ設定 × 全 `.scm` フィクスチャファイル
  - テスト設定: JIT無効、JIT最適化無効、各種ブロックフュージョン組み合わせ
  - スナップショット: `e2e_snapshots/` に stdout/stderr/exitCode を記録
  - CI ではシャーディング (4分割) で並列実行
- **ベンチマーク**: `tinybench` によるベンチマーク (tak, div2, matmul 等)
  - Gauche / Guile Hoot との比較
  - ウォームアップモード: none/static/dynamic
- **Debug assertions**: `debug_assert_ssa`, `debug_assert_phi_rules` 等で内部不変条件を検証

## コーディング規約

### Rust
- エラー型: `CompilerError(String)` + `compiler_error!` マクロ
- ID 型 (`FuncId`, `LocalId`, `GlobalId`, `BasicBlockId` 等) は `VecMap` のキーとして使用
- AST は "Trees that Grow" パターン (`AstPhase` トレイト + associated types) でフェーズごとに型安全に変換
- IR の命令は `InstrKind` enum (100+ variants)
- SSA 形式の IR を構築・検証・最適化・脱構築する
- `FxHashMap` / `FxBiHashMap` を高頻度ルックアップに使用

### TypeScript
- ESM (`"type": "module"`)
- `WebAssembly.Module` の同期的インスタンス化 (`js_instantiate` コールバック)
- `dynamic` オブジェクトによる動的モジュールリンク
- Worker によるバックグラウンド実行 (Playground)

### WebAssembly
- Wasm GC の struct/array で Scheme の値を表現 (Nil, Bool, Int, Float, String, Symbol, Cons, Vector, Closure 等)
- 文字列は Copy-on-Write (`StringBuf.shared` フラグ)
- リニアメモリは Rust ヒープと GC↔リニアメモリ間データ転送にのみ使用

## R5RS からの逸脱

- 継続 (continuation) は未実装
- 整数オーバーフローはチェックしない (ラップアラウンド)
- 組み込み手続きの `set!`/`define` による再定義はエラー
- UVector (s64vector, f64vector) は R5RS 外の拡張 (数値ベンチマーク用)

## CI/CD

- GitHub Actions: `ci.yaml` (ビルド・テスト・ベンチマーク・デプロイ)、`autofix.yaml` (自動フォーマット)、`devcontainer.yaml` (コンテナイメージビルド)
- GitHub Pages にPlayground + ベンチマーク結果をデプロイ (master ブランチのみ)
- マルチアーキテクチャ対応 (amd64 + arm64)

## デバッグ

- `LOG_STDOUT=1`: `log::debug!` 出力をランタイム/コンパイラから有効化
- `LOG=1`: 生成された IR と Wasm バイナリを `webschembly-js/log/` にダンプ
- 実行例: `cd webschembly-js && just LOG_STDOUT=1 run ./fixtures/rec.scm`
- IR ファイルはタイムスタンプ付きで、module_id で JIT モジュールと対応
