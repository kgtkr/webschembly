# Rust 開発ルール

## ツールチェイン
- **Rust nightly-2025-10-09** を使用 (`rust-toolchain.toml` で固定)
- Edition 2024
- ターゲット: ネイティブ (CLI用) + `wasm32-unknown-unknown` (ランタイム用)
- rust-analyzer は `.vscode/settings.json` で `wasm32-unknown-unknown` をデフォルトターゲットに設定

## Nightly Features (使用中)
以下の unstable features をクレートで使用している:
- `string_deref_patterns`
- `box_patterns`
- `trait_alias`
- `never_type` (`!` 型 — AST の拡張ポイントで使用)
- `coroutines`
- `if_let_guard`
- `impl_trait_in_assoc_type`

## Cargo Workspace 構成
- ルート `Cargo.toml` でワークスペースを定義
- メンバー: `webschembly-compiler`, `webschembly-compiler-cli`, `webschembly-compiler-crates/*`, `webschembly-runtime-rust`
- resolver v2
- 共有依存関係: `log`, `ordered-float`, `rustc-hash`, `strum`, `strum_macros`

## コード規約

### エラーハンドリング
- ユーザー向けエラー: `CompilerError(String)` + `compiler_error!` マクロ (`webschembly-compiler-crates/error`)
- 型: `type Result<T> = std::result::Result<T, CompilerError>`
- 内部不変条件違反: `panic!` / `debug_assert!` を使用 (ユーザー向けではない)
- Lexer エラー: `nom::error::VerboseError` → `CompilerError` に変換

### ID 型と VecMap
- `FuncId`, `LocalId`, `GlobalId`, `BasicBlockId`, `JitModuleId`, `JitFuncId`, `JitBasicBlockId` 等の newtype ID 型
- `VecMap<K, V>` (カスタム疎ベクタマップ) のキーとして使用
- `HasId` トレイトで ID 生成を抽象化
- `vec-map` クレートの `test-util` feature で `PartialEq`/`Eq` derives を有効化

### AST 設計パターン — "Trees that Grow"
- `AstPhase` トレイトに associated types (`XConst`, `XVar`, `XLambda`, `XIf`, `XCall`, `XSet`, `XExt` 等) を定義
- 各コンパイルフェーズで AST 型を段階的に狭める:
  - `Parsed` → `Desugared` → `Defined` → `TailCall` → `Used`
- 脱糖済みの構文は extension type を `!` (never type) にして型レベルで排除
- 最終 AST 型: `type Final = Used<TailCall<Defined<Desugared<Parsed>>>>`

### IR 設計
- SSA 形式の IR を構築
- `InstrKind` enum は 100+ variants (Nop, Phi, Bool, Int, Call, Closure, ToObj, FromObj 等)
- `TerminatorInstr`: If, Jump, Exit
- `ExitInstr`: Return, TailCall, TailCallRef, TailCallClosure, Error
- `Type`: Obj | Val(ValType) — Scheme の値はランタイムでは Obj (Wasm GC anyref) としてボックス化
- `debug_assert_ssa`, `debug_assert_phi_rules` で SSA 不変条件を検証

### ハッシュマップ
- `FxHashMap` / `FxBiHashMap` を高頻度ルックアップに使用 (暗号学的ハッシュ不要な場面)

## テスト
- `insta` クレートによるスナップショットテスト (lexer)
- `debug_assert!` ベースの不変条件チェック
- `cargo test` で全ユニットテストを実行

## フォーマット
- `rustfmt` (edition 2024 設定) — `treefmt` 経由で実行
- `cargo clippy` — CI で実行、`autofix.yaml` ワークフローで自動修正
