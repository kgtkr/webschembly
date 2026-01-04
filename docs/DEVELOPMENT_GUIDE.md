# 開発ガイド

## 目次
1. [開発環境のセットアップ](#1-開発環境のセットアップ)
2. [コードベースの構造](#2-コードベースの構造)
3. [開発ワークフロー](#3-開発ワークフロー)
4. [実装例: 新しい最適化の追加](#4-実装例-新しい最適化の追加)
5. [テストとデバッグ](#5-テストとデバッグ)
6. [ベンチマーク](#6-ベンチマーク)
7. [トラブルシューティング](#7-トラブルシューティング)

---

## 1. 開発環境のセットアップ

### 1.1 必要なツール

- **Nix**: パッケージ管理・開発環境構築
- **direnv**: 自動環境切り替え
- **Rust**: 1.70以上（rust-toolchain.tomlで管理）
- **Node.js**: JavaScriptバインディング用

### 1.2 初期セットアップ

```bash
# リポジトリのクローン
git clone https://github.com/kgtkr/webschembly.git
cd webschembly

# direnvの有効化
direnv allow

# Nixシェルに入る（direnvが自動で行う）
# または手動で:
nix-shell

# ビルド確認
cargo build --all
```

### 1.3 開発ツールのインストール

```bash
# Rustフォーマッタ
cargo install rustfmt

# Clippy（リンター）
rustup component add clippy

# Wasm関連ツール
cargo install wasm-pack
cargo install wasm-bindgen-cli
```

---

## 2. コードベースの構造

### 2.1 ディレクトリ構成

```
webschembly/
├── webschembly-compiler/           # コンパイラコア
│   ├── src/
│   │   ├── compiler.rs             # メインコンパイラロジック
│   │   ├── ir_generator/           # IR生成
│   │   ├── ir_processor/           # IR最適化
│   │   ├── jit/                    # JITシステム
│   │   │   ├── mod.rs              # JITエントリポイント
│   │   │   ├── jit_func.rs         # 関数JITコンパイル
│   │   │   ├── jit_ctx.rs          # JITコンテキスト
│   │   │   └── global_layout.rs    # グローバルレイアウト
│   │   ├── wasm_generator/         # Wasm生成
│   │   └── stdlib.scm              # 標準ライブラリ
│   └── Cargo.toml
│
├── webschembly-compiler-crates/    # コンパイラサブクレート
│   ├── ast/                        # AST定義
│   ├── ir/                         # IR定義
│   ├── sexpr/                      # S式表現
│   └── ...
│
├── webschembly-runtime-rust/       # Rustランタイム
│   └── src/lib.rs                  # ランタイムAPI
│
├── webschembly-js/                 # JavaScriptバインディング
│   ├── src/                        # TypeScriptコード
│   ├── fixtures/                   # テストケース
│   └── package.json
│
├── webschembly-playground/         # Webプレイグラウンド
│   └── src/
│
├── docs/                           # ドキュメント
│   ├── ARCHITECTURE.md             # アーキテクチャ詳細
│   ├── DEVELOPMENT_GUIDE.md        # 本ドキュメント
│   └── *.tex                       # 論文用図表
│
└── README.md                       # プロジェクト概要
```

### 2.2 主要モジュールの役割

| モジュール | 責務 | 主な型 |
|-----------|------|--------|
| `ir_generator` | SchemeコードをIRに変換 | `IrGenerator` |
| `ir_processor` | IR最適化・解析 | `Optimizer`, `DefUseChain` |
| `jit` | 動的コンパイル | `Jit`, `JitSpecializedFunc` |
| `wasm_generator` | Wasmバイナリ生成 | `WasmGenerator` |
| `runtime-rust` | ランタイムサポート | API関数群 |

---

## 3. 開発ワークフロー

### 3.1 典型的な開発サイクル

```bash
# 1. ブランチ作成
git checkout -b feature/new-optimization

# 2. コード編集
vim webschembly-compiler/src/jit/new_optimization.rs

# 3. ビルド＆テスト
cargo build
cargo test

# 4. フォーマット＆Lint
cargo fmt
cargo clippy

# 5. 統合テスト（JS側）
cd webschembly-js
npm test

# 6. ベンチマーク
just benchmark

# 7. コミット＆プッシュ
git add .
git commit -m "feat: Add new optimization"
git push origin feature/new-optimization
```

### 3.2 ビルドコマンド

```bash
# デバッグビルド
cargo build

# リリースビルド
cargo build --release

# 特定のクレートのみビルド
cargo build -p webschembly-compiler
cargo build -p webschembly-runtime-rust

# Wasmターゲットへのビルド
cargo build --target wasm32-unknown-unknown
```

### 3.3 テスト実行

```bash
# Rustのユニットテスト
cargo test

# 特定のテストのみ実行
cargo test test_name

# 統合テスト（JavaScript側）
cd webschembly-js
npm test

# 特定のフィクスチャのみテスト
npm test -- --grep "fibonacci"
```

---

## 4. 実装例: 新しい最適化の追加

### 4.1 シナリオ: Polymorphic Inline Caching (PIC)

高階関数の呼び出しを高速化するために、PICを実装する例。

#### ステップ1: IR命令の拡張

**ファイル**: `webschembly-compiler-crates/ir/src/lib.rs`

```rust
// IR命令に新しいバリアントを追加
pub enum InstrKind {
    // 既存の命令...
    
    // 新規: PICサイト
    CallClosurePIC {
        closure: LocalId,
        args: Vec<LocalId>,
        cache: PICCache,  // キャッシュエントリ
    },
}

// PICキャッシュの定義
#[derive(Debug, Clone)]
pub struct PICCache {
    pub site_id: usize,           // 呼び出しサイトID
    pub cached_types: Vec<Type>,  // キャッシュされた型
    pub cached_target: FuncId,    // キャッシュされたターゲット
}
```

#### ステップ2: JIT側の実装

**ファイル**: `webschembly-compiler/src/jit/pic.rs` (新規作成)

```rust
use rustc_hash::FxHashMap;
use webschembly_compiler_ir::*;

pub struct PICManager {
    // サイトID → 型プロファイル
    type_profiles: FxHashMap<usize, Vec<TypeProfile>>,
}

#[derive(Debug, Clone)]
struct TypeProfile {
    types: Vec<Type>,
    target: FuncId,
    count: usize,  // 実行回数
}

impl PICManager {
    pub fn new() -> Self {
        Self {
            type_profiles: FxHashMap::default(),
        }
    }
    
    pub fn record_call(
        &mut self,
        site_id: usize,
        types: Vec<Type>,
        target: FuncId,
    ) {
        let profile = self.type_profiles
            .entry(site_id)
            .or_insert_with(Vec::new);
        
        // 既存のプロファイルを更新
        if let Some(entry) = profile.iter_mut()
            .find(|p| p.types == types && p.target == target)
        {
            entry.count += 1;
        } else {
            profile.push(TypeProfile {
                types,
                target,
                count: 1,
            });
        }
    }
    
    pub fn get_hot_targets(&self, site_id: usize, threshold: usize) 
        -> Vec<(Vec<Type>, FuncId)> 
    {
        self.type_profiles
            .get(&site_id)
            .map(|profiles| {
                profiles.iter()
                    .filter(|p| p.count >= threshold)
                    .map(|p| (p.types.clone(), p.target))
                    .collect()
            })
            .unwrap_or_default()
    }
}
```

#### ステップ3: コード生成の変更

**ファイル**: `webschembly-compiler/src/jit/jit_func.rs`

```rust
impl JitSpecializedFunc {
    pub fn generate_bb_module(
        &mut self,
        // 既存のパラメータ...
        pic_manager: &PICManager,  // 新規パラメータ
    ) -> (Module, FuncId) {
        // ... 既存のコード ...
        
        for instr in &body_func.bbs[bb_id].instrs {
            match &instr.kind {
                // 通常のクロージャ呼び出し
                InstrKind::CallClosure(call_closure) => {
                    // PICの機会をチェック
                    if let Some(hot_targets) = 
                        pic_manager.get_hot_targets(call_closure.site_id, 10)
                    {
                        // PIC付き呼び出しに変換
                        self.generate_pic_call(
                            call_closure,
                            hot_targets,
                            &mut instrs,
                        );
                    } else {
                        // 通常の呼び出し
                        instrs.push(instr.clone());
                    }
                }
                _ => {
                    instrs.push(instr.clone());
                }
            }
        }
        
        // ... 既存のコード ...
    }
    
    fn generate_pic_call(
        &mut self,
        call: &InstrCallClosure,
        hot_targets: Vec<(Vec<Type>, FuncId)>,
        instrs: &mut Vec<Instr>,
    ) {
        // PICロジック:
        // 1. 引数の型をチェック
        // 2. キャッシュにヒットしたら直接呼び出し
        // 3. ミスしたら通常の間接呼び出し
        
        for (cached_types, target_func) in hot_targets {
            // 型チェックコードの生成
            // if (arg1.type == cached_types[0] && 
            //     arg2.type == cached_types[1] && ...) {
            //     return target_func(arg1, arg2, ...);
            // }
            
            // ... コード生成ロジック ...
        }
        
        // フォールバック: 通常の間接呼び出し
        instrs.push(Instr {
            local: instr.local,
            kind: InstrKind::CallClosure(call.clone()),
        });
    }
}
```

#### ステップ4: ランタイムサポート

**ファイル**: `webschembly-runtime-rust/src/lib.rs`

```rust
// PICプロファイリングAPI
#[unsafe(no_mangle)]
pub extern "C" fn record_pic_call(
    site_id: i32,
    type_count: i32,
    types_ptr: i32,  // Type配列へのポインタ
    target_func: i32,
) {
    COMPILER.with(|compiler| {
        let mut compiler = compiler.borrow_mut();
        let compiler = compiler.as_mut().unwrap();
        
        // 型情報の読み取り
        let types = unsafe {
            std::slice::from_raw_parts(
                types_ptr as *const i32,
                type_count as usize,
            )
        };
        
        // PICマネージャーに記録
        compiler.pic_manager.record_call(
            site_id as usize,
            types.iter().map(|&t| decode_type(t)).collect(),
            FuncId::from(target_func as usize),
        );
    });
}
```

#### ステップ5: テストケースの追加

**ファイル**: `webschembly-js/fixtures/pic_test.scm`

```scheme
;; 高階関数のテスト
(define (map f lst)
  (if (null? lst)
      '()
      (cons (f (car lst))
            (map f (cdr lst)))))

;; 整数リストへの適用
(define int-list '(1 2 3 4 5))
(define (double x) (* x 2))
(write (map double int-list))  ;; => (2 4 6 8 10)

;; 浮動小数点リストへの適用（PICが2つのケースをキャッシュ）
(define float-list '(1.0 2.0 3.0 4.0 5.0))
(define (half x) (/ x 2))
(write (map half float-list))  ;; => (0.5 1.0 1.5 2.0 2.5)
```

#### ステップ6: ベンチマーク

**ファイル**: `webschembly-js/benchmark.ts`

```typescript
// PIC効果の測定
benchmark('map-with-pic', () => {
  const code = `
    (define (map f lst)
      (if (null? lst)
          '()
          (cons (f (car lst))
                (map f (cdr lst)))))
    
    (define (double x) (* x 2))
    (define long-list (iota 10000))
    (map double long-list)
  `;
  
  const result = eval(code);
  return result;
});
```

---

## 5. テストとデバッグ

### 5.1 ユニットテスト

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pic_cache_hit() {
        let mut pic_manager = PICManager::new();
        
        // 同じ型で複数回呼び出し
        for _ in 0..15 {
            pic_manager.record_call(
                1,  // site_id
                vec![Type::Val(ValType::Int)],
                FuncId::from(42),
            );
        }
        
        // ホットターゲットとして検出されるべき
        let hot = pic_manager.get_hot_targets(1, 10);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].1, FuncId::from(42));
    }
}
```

### 5.2 統合テスト

```bash
cd webschembly-js
npm test
```

### 5.3 デバッグ技法

#### IRダンプの有効化

```rust
// compiler.rs
if cfg!(debug_assertions) {
    let ir_dump = format!("{}", module.display());
    eprintln!("=== IR Dump ===\n{}", ir_dump);
}
```

#### ログ出力

```rust
use log::{debug, info, warn};

debug!("PIC site {} hit count: {}", site_id, count);
info!("Compiling optimized trace: BB{} -> BB{}", start, end);
warn!("PIC cache miss at site {}", site_id);
```

#### Wasmバイナリの検査

```bash
# Wasmをテキスト形式に変換
wasm2wat output.wasm -o output.wat

# または
wasm-objdump -d output.wasm
```

#### GDB/LLDBでのデバッグ

```bash
# Rustコードのデバッグ
rust-gdb target/debug/webschembly-compiler-cli
(gdb) break compiler.rs:123
(gdb) run test.scm
```

---

## 6. ベンチマーク

### 6.1 ベンチマーク実行

```bash
cd webschembly-js
just benchmark
```

### 6.2 カスタムベンチマークの追加

**ファイル**: `webschembly-js/src/benchmark.ts`

```typescript
import { benchmark, Suite } from './benchmark-framework';

const suite = new Suite('My Optimization');

suite.add('baseline', () => {
  // ベースライン実装
  const code = `(+ 1 2 3)`;
  return eval(code);
});

suite.add('optimized', () => {
  // 最適化版
  const code = `(+ 1 2 3)`;
  return evalWithOptimization(code);
});

suite.run();
```

### 6.3 パフォーマンスプロファイリング

#### ブラウザDevTools

```javascript
// Chrome DevTools Performance タブでプロファイル
console.profile('JIT Compilation');
eval(schemeCode);
console.profileEnd('JIT Compilation');
```

#### Node.js Profiler

```bash
node --prof webschembly-js/dist/cli.js test.scm
node --prof-process isolate-*.log > profile.txt
```

---

## 7. トラブルシューティング

### 7.1 よくあるエラー

#### エラー: `global not found`

**原因**: グローバル変数のインデックスが未初期化

**解決**:
```rust
// global_layout.rs で適切なインデックスを割り当て
let (index, flag) = jit_ctx.closure_global_layout()
    .idx(&closure_args)
    .expect("Global index not allocated");
```

#### エラー: `SSA violation`

**原因**: ローカル変数への複数回の代入

**解決**:
```rust
// build_ssa を呼び出してSSA形式に変換
let new_ids = build_ssa(func);
```

#### エラー: `unreachable BB`

**原因**: 到達不能な基本ブロックが残存

**解決**:
```rust
// remove_unreachable_bb で削除
remove_unreachable_bb(func);
```

### 7.2 デバッグチェックリスト

- [ ] IRが正しいSSA形式か？ (`debug_assert_ssa`)
- [ ] 到達不能BBは削除されているか？
- [ ] グローバル変数は初期化されているか？
- [ ] 型情報は正しく伝搬しているか？
- [ ] Phi命令の引数は正しいか？

### 7.3 パフォーマンスが悪い場合

1. **IRダンプを確認**: 不要な命令が残っていないか
2. **最適化フラグ**: JITConfigで最適化が有効か確認
3. **ベンチマーク**: どの部分がボトルネックか特定
4. **プロファイリング**: CPU時間の消費箇所を特定

---

## 8. リリースプロセス

### 8.1 リリース前チェック

```bash
# 1. 全テストが通ることを確認
cargo test --all
cd webschembly-js && npm test

# 2. Lintチェック
cargo clippy -- -D warnings
npm run lint

# 3. ベンチマーク結果を確認
just benchmark

# 4. ドキュメント生成
cargo doc --no-deps
```

### 8.2 バージョンアップ

```bash
# Cargo.tomlのバージョンを更新
vim webschembly-compiler/Cargo.toml

# package.jsonのバージョンを更新
vim webschembly-js/package.json

# Git タグ
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

---

## 9. 参考リソース

### 9.1 内部ドキュメント

- [ARCHITECTURE.md](./ARCHITECTURE.md): システムアーキテクチャ詳細
- [r5rs.md](./r5rs.md): R5RS準拠状況
- [function_jit.md](./function_jit.md): JIT実装詳細

### 9.2 外部リソース

- [WebAssembly仕様](https://webassembly.github.io/spec/)
- [Wasm GC提案](https://github.com/WebAssembly/gc)
- [SSAアルゴリズム](https://en.wikipedia.org/wiki/Static_single_assignment_form)
- [Relooperアルゴリズム](https://github.com/emscripten-core/emscripten/blob/main/docs/paper.pdf)

### 9.3 論文・記事

- "Trace-based Just-in-Time Type Specialization for Dynamic Languages" (Gal et al.)
- "Polymorphic Inline Caching" (Hölzle et al.)
- "Emscripten: An LLVM-to-JavaScript Compiler" (Zakai)

---

このガイドを使って、効率的に開発を進めてください。
質問や問題があれば、GitHubのIssueで報告してください。
