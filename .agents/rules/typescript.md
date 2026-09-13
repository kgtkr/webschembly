# TypeScript & JS 環境開発ルール

## ワークスペースと設定

- Node.js 22 を使用 (`js.nix` 設定)
- npm workspaces を使用 (`webschembly-js`, `webschembly-playground`)
- ESM (`"type": "module"`)
- `tsconfig.base.json`:
  - `target: ES2020`
  - `module: ESNext`
  - `moduleResolution: node`
  - Strict mode
  - `verbatimModuleSyntax: true`
  - Composite projects

## WebAssembly の同期インスタンス化

- `WebAssembly.Module` のインスタンス化は同期的 (`js_instantiate` コールバック)
- コンパイル中の Wasm モジュール生成時に、Rust から呼び出される
- `dynamic` オブジェクトを共有参照として渡し、動的モジュールリンクを実現する
  - `js_instantiate` で新しく生成された Wasm の exports を `dynamic` オブジェクトに `Object.assign` でマージする
  - これにより、後続のモジュールが以前のモジュールの関数を `dynamic` 名前空間経由で呼び出せる

## Wasm-JS インターフェース (webschembly-js)

- `createRuntime()` が中心となる JS-Wasm ブリッジ
- `RuntimeImportsEnv` で 4 つの JS コールバックを提供:
  1. `js_instantiate`: 同期的モジュールインスタンス化
  2. `js_webschembly_log`: ログ出力
  3. `js_webschembly_jit_log`: JIT ログ出力 (JSON パース)
  4. `js_write_buf`: I/O バッファ書き込み
- コンパイラ設定 (`CompilerConfig`) を JS 側から渡し、Wasm の `init()` を呼ぶ
- `WEBSCHEMBLY_EXCEPTION` タグをキャッチして適切にエラーハンドリングする

## Playground (React)

- Vite + React 19
- `@xyflow/react` + `dagre` による JIT 状態の可視化グラフ (`JitGraph.tsx`)
- Web Worker (`playground.worker.ts`) によるバックグラウンド実行
  - Wasm モジュールの実行は Worker 内で行い、メインスレッドをブロックしない
  - メッセージパッシングで stdout/stderr/exitCode/duration と JIT ログイベントを転送
- ダークテーマ、Glassmorphism を基調とした CSS デザイン (`src/index.css`)

## テスト (webschembly-js)

- `vitest` によるスナップショットテスト (`e2e.test.ts`)
  - 5つのコンパイラ設定 × 全 `.scm` フィクスチャファイルで実行
  - 設定: no JIT optimization, no JIT, 各種ブロックフュージョン組み合わせ
  - stdout, stderr, exitCode をキャプチャし、`e2e_snapshots/` と比較
  - CI では 4 分割のシャーディング実行

## ベンチマーク (webschembly-js)

- `tinybench` ライブラリを使用 (`e2e.benchmark.ts`)
- `.b.scm` (tak, div2, matmul 等) を対象
- 複数のコンパイラ設定 × ウォームアップモード (none/static/dynamic)
- "Dynamic warmup" モードでは、計測前に関数を 30 回実行し JIT コンパイルをトリガーする
- Guile Hoot との比較ベンチマーク対応

## フォーマット

- `dprint` によるフォーマット (`dprint.json`)
  - TS/JS/JSON/MD/TOML/CSS/HTML/YAML 対応
  - `treefmt` 経由で実行
