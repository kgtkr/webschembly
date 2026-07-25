# ビルドと開発環境 (Nix) のルール

## 概要
プロジェクト全体のオーケストレーションは Nix (`flake-parts`) で行われる。すべてのツールチェインと依存関係は Nix 経由で提供される。

## 開発環境 (`dev shell`)
- `direnv` をサポート (`.envrc` で `use flake`)
- `nix develop` または direnv で自動でシェルに入る
- 提供される主なツール: `rust-toolchain`, `gnumake`, `just`, `binaryen`, `wasm-tools`, `nixpkgs-fmt`, `treefmt`, `dprint`, `schemat`, Node.js など
- `.devcontainer/` で同等の環境を VS Code / GitHub Codespaces にも提供 (`devcontainer.nix` で構築)

## Rust のビルド (`rust.nix`)
- `cargo2nix-ifd` を使用して Cargo プロジェクトを Nix derivation に変換
- 以下の3種類のパッケージセットを構築:
  1. `staticRustPkgs`: CLI 用 (x86_64 / aarch64 ネイティブ)
  2. `wasmRustPkgs`: ランタイム用 Release ビルド (`wasm32-unknown-unknown`)
  3. `debugWasmRustPkgs`: テスト用 Debug ビルド (`wasm32-unknown-unknown`)
- `webschembly-runtime` のビルドには、Rust の wasm 出力と `lib.wat` を統合する Makefile 処理が Nix 内部で実行される

## JavaScript/Playground のビルド (`js.nix`)
- `napalm` を使用して npm パッケージのビルドを Nix 化
- `webschembly-playground`: Vite による React アプリのビルド。`wasm` ファイルを静的アセットとしてコピーする
- `webschembly-playground-for-pages`: GitHub Pages デプロイ用に `BASE_URL` 環境変数を設定したビルド

## Scheme フォーマッタ (`schemat.nix`)
- Scheme および WAT ファイルのフォーマット用ツール `schemat` (GitHub からソースを取得して Rust ビルド)

## CI ワークフロー (`.github/workflows/`)
- `ci.yaml`: フォーマットチェック (`treefmt --ci`)、Clippy、Rust テスト、JS/TS のテストとビルドを実行
- シャーディング実行 (テストとベンチマーク) で高速化
- `autofix.yaml`: 変更に対して自動で `treefmt` と `cargo clippy --fix` を適用してコミット
- `devcontainer.yaml`: `skopeo` を使用して OCI イメージをマルチアーキテクチャ (amd64, arm64) で GitHub Container Registry にプッシュ
