# Webschembly アーキテクチャ詳細

## 1. プロジェクト概要

### 1.1 目標
R5RS Schemeを対象とした、WebAssembly上で動作する高速な動的JITコンパイラの実現。

### 1.2 評価指標
- **主要指標**: ウォームアップ後のピーク性能（対 AOTコンパイラ比）
- **現状**: AOTコンパイラの約1.5倍遅い（Tier 2実装後、一部でAOT超え達成）

### 1.3 技術的課題
- **解決済み**: 制御フロー（ジャンプ）のオーバーヘッド → Tier 2線形化JITで解決
- **残存課題**: 
  - データアクセスのコスト
  - 関数呼び出し（特に高階関数）のコスト
  - 動的言語特有の機能（継続など）のコスト

### 1.4 今後
線形化JITに加え、もう一つの強力な最適化手法が欲しい

---

## 2. システムアーキテクチャ

### 2.1 全体構成図

```
┌─────────────────────────────────────────────────────────────┐
│                     Scheme ソースコード                       │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    Parser & Lexer                           │
│  (webschembly-compiler/src/sexpr_parser.rs)                │
│  (webschembly-compiler/src/lexer/)                         │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                   IR Generator                              │
│  (webschembly-compiler/src/ir_generator/)                  │
│  - SSA形式への変換                                            │
│  - 型情報の付加                                               │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                   IR Processor                              │
│  (webschembly-compiler/src/ir_processor/)                  │
│  - SSA最適化                                                 │
│  - データフロー解析                                            │
│  - インライン化                                               │
└─────────────────────┬───────────────────────────────────────┘
                      │
                ┌─────┴─────┐
                │             │
                ▼             ▼
    ┌───────────────┐  ┌──────────────┐
    │  AOT Path     │  │  JIT Path    │
    │  (wasm_gen)   │  │  (jit/)      │
    └───────┬───────┘  └──────┬───────┘
            │                  │
            ▼                  ▼
    ┌───────────────┐  ┌──────────────┐
    │  Static Wasm  │  │ Dynamic Wasm │
    │  Module       │  │ Modules      │
    └───────────────┘  └──────┬───────┘
                               │
                               ▼
                    ┌──────────────────┐
                    │  Runtime System  │
                    │  (runtime-rust)  │
                    └──────────────────┘
```

### 2.2 モジュール構成

#### 2.2.1 コンパイラコア (`webschembly-compiler/`)
- **lib.rs**: コンパイラのエントリポイント
- **compiler.rs**: コンパイルパイプライン全体の制御
- **parser_combinator.rs**: パーサーコンビネータ実装
- **sexpr_parser.rs**: S式パーサー
- **token.rs / tokens.rs**: トークン定義

#### 2.2.2 IR生成 (`ir_generator/`)
- **機能**: SchemeのASTから中間表現（IR）への変換
- **特徴**: 
  - SSA形式の生成
  - 型情報の伝搬
  - クロージャの処理

#### 2.2.3 IR処理 (`ir_processor/`)
- **ssa.rs**: SSA変換・最適化
- **cfg_analyzer.rs**: 制御フローグラフ解析
- **dataflow.rs**: データフロー解析（def-use解析、生存区間解析）
- **optimizer.rs**: 基本的な最適化パス
- **ssa_optimizer.rs**: SSA特化の最適化

#### 2.2.4 JITシステム (`jit/`)
- **mod.rs**: JITシステムのメインロジック
- **jit_config.rs**: JIT設定
- **jit_ctx.rs**: JITコンテキスト（グローバル状態管理）
- **jit_func.rs**: 関数・基本ブロック単位のJITコンパイル
- **jit_module.rs**: JITモジュール管理
- **global_layout.rs**: グローバル変数レイアウト管理

#### 2.2.5 Wasm生成 (`wasm_generator/`)
- **機能**: IRからWebAssemblyバイナリへの変換
- **relooper.rs**: 制御フロー構造化アルゴリズム

#### 2.2.6 ランタイム (`webschembly-runtime-rust/`)
- **lib.rs**: Rustで実装されたランタイム関数
- **機能**:
  - JITコンパイラのホスティング
  - メモリ管理
  - I/O処理
  - プロファイリング・カウンタ管理

---

## 3. JITシステムの詳細設計

### 3.1 実行モデル: 2段階Tieringシステム

```
┌────────────────────────────────────────────────────────┐
│                    Tier 1: Baseline JIT                │
│                                                        │
│  - 基本ブロック単位でコンパイル                           │
│  - Global Dispatch方式による遅延コンパイル                │
│  - Basic Block Versioning (BBV)                       │
└────────────────────────────────────────────────────────┘
                         │
                         │ プロファイリング
                         │ (branch counter)
                         ▼
┌────────────────────────────────────────────────────────┐
│                 Tier 2: Optimizing JIT                │
│                                                        │
│  - Trace Linearization（トレース線形化）                 │
│  - ホットパスの検出と最適化                               │
│  - 複数BBの結合・インライン展開                           │
└────────────────────────────────────────────────────────┘
```

### 3.2 Tier 1: Baseline JIT

#### 3.2.1 Global Dispatch機構

**コンセプト**: WebAssemblyの `global (mut funcref)` を「関数ポインタテーブル」として活用

```wasm
;; グローバル変数の定義例
(global $bb_0_v0 (mut funcref) (ref.func $stub_bb_0_v0))
(global $bb_0_v1 (mut funcref) (ref.func $stub_bb_0_v1))
(global $bb_1_v0 (mut funcref) (ref.func $stub_bb_1_v0))
```

**動作メカニズム**:
1. 各基本ブロック（BB）は1つのWasm関数として実装
2. BB間の遷移は `return_call_ref` を使用
3. 初回はスタブ関数が登録されている
4. スタブが呼ばれるとコンパイラを起動し、実コードを生成
5. グローバル変数を書き換えて、次回から実コードが実行される

```rust
// jit_func.rs の generate_bb_module メソッド
// BB遷移のコード生成例
Instr {
    local: Some(func_ref_local),
    kind: InstrKind::GlobalGet(index_global.id),  // funcrefを取得
}
Instr {
    local: None,
    kind: InstrKind::Terminator(TerminatorInstr::Exit(
        ExitInstr::TailCallRef(InstrCallRef {
            func: func_ref_local,
            args: locals_to_pass,
            func_type: FuncType { ... },
        })
    )),
}
```

#### 3.2.2 Basic Block Versioning (BBV)

**目的**: 引数の型情報に基づいた特殊化により、動的型チェックを削減

**実装詳細**:
- 同じ論理的BBでも、異なる型構成の引数に対して別々のグローバル変数を割り当て
- 型情報は `LocalType` として伝搬
- 型が確定している場合、`from_obj` 命令で明示的に型変換

```rust
// jit_func.rs の型特殊化処理
let mut typed_objs = FxHashMap::default();
for (obj_local, typ) in types {
    let val_local = body_func.locals.push_with(|id| Local {
        id,
        typ: typ.into(),
    });
    instrs.push(Instr {
        local: Some(val_local),
        kind: InstrKind::FromObj(obj_local, typ),
    });
    typed_objs.insert(obj_local, val_local);
}
```

#### 3.2.3 遅延コンパイル（Lazy Compilation）

**スタブ関数の役割**:
```rust
// スタブ関数の生成（簡略化）
Func {
    id: stub_func_id,
    args: args_for_stub,
    ret_type: LocalType::FuncRef,
    locals,
    bb_entry: BasicBlockId::from(0),
    bbs: [BasicBlock {
        id: BasicBlockId::from(0),
        instrs: vec![
            // コンパイラを呼び出す
            Instr {
                local: Some(func_ref_local),
                kind: InstrKind::InstantiateFunc(module_id, func_id, closure_idx),
            },
            // グローバル変数を更新
            Instr {
                local: Some(mut_func_ref_local),
                kind: InstrKind::CreateMutFuncRef(func_ref_local),
            },
            Instr {
                local: None,
                kind: InstrKind::SetEntrypointTable(
                    closure_idx,
                    entrypoint_table_local,
                    mut_func_ref_local,
                ),
            },
            // 新しく生成された関数を返す
            Instr {
                local: None,
                kind: InstrKind::Terminator(TerminatorInstr::Exit(
                    ExitInstr::Return(func_ref_local)
                )),
            },
        ],
    }].into_iter().collect(),
}
```

### 3.3 Tier 2: Optimizing JIT (Trace Linearization)

#### 3.3.1 プロファイリング機構

**ブランチカウンタの埋め込み**:
```rust
// jit_func.rs の generate_bb_module
if !branch_specialization {
    instrs.push(Instr {
        local: None,
        kind: InstrKind::IncrementBranchCounter(
            self.module_id,
            JitFuncId::from(self.func.id),
            self.func_index,
            JitBasicBlockId::from(bb_id),
            branch_kind,  // Then or Else
            JitBasicBlockId::from(orig_entry_bb_id),
            index,
        ),
    });
}
```

**カウンタ管理** (runtime-rust/lib.rs):
```rust
pub extern "C" fn increment_branch_counter(
    module_id: i32,
    func_id: i32,
    func_index: i32,
    bb_id: i32,
    kind: i32, // 0: Then, 1: Else
    source_bb_id: i32,
    source_index: i32,
) {
    // カウンタをインクリメント
    // しきい値を超えたら最適化コンパイルをトリガー
    let wasm_ir = compiler.increment_branch_counter(...);
    // 生成されたWasmモジュールをインスタンス化
    if let Some((wasm, ir)) = wasm_ir {
        unsafe { env::js_instantiate(...) }
    }
}
```

#### 3.3.2 Trace Linearization（トレース線形化）

**アルゴリズム**:
1. **ホットパス検出**: branch counterがしきい値を超えた経路を特定
2. **BB結合**: 連続するBBをマージ（例: BB1 → BB2 → ... → BB10）
3. **制御フロー最適化**: `return_call_ref` の連鎖を `block/loop/br` に置換
4. **インライン展開**: 呼び出し先関数の本体を直接埋め込み
5. **Promotion**: エントリポイントBBのグローバル変数を新しい関数で上書き

**実装** (jit_func.rs):
```rust
pub fn generate_bb_module(
    &mut self,
    func_to_globals: &VecMap<FuncId, GlobalId>,
    func_types: &VecMap<FuncId, FuncType>,
    orig_entry_bb_id: BasicBlockId,
    index: usize,
    global_manager: &mut GlobalManager,
    jit_ctx: &mut JitCtx,
    branch_specialization: bool,  // Tier 2の場合true
) -> (Module, FuncId) {
    // BBのマージ処理
    let mut processed_bb_ids = FxHashSet::default();
    let mut todo_bb_ids = vec![orig_entry_bb_id];
    
    while let Some(bb_id) = todo_bb_ids.pop() {
        // BBの命令を結合
        // 分岐の特殊化（branch_specialization = true）
        // ...
    }
    
    // SSA最適化
    let new_ids = build_ssa(body_func);
    
    // デッドコード除去、定数伝搬など
    if jit_ctx.config().enable_optimization {
        ssa_optimize(body_func, SsaOptimizerConfig { ... });
    }
    
    // ...
}
```

#### 3.3.3 Promotion（昇格）

**メカニズム**: 最適化された関数でグローバル変数を上書き

```
Before:
  global $bb_1 = funcref(baseline_bb_1)

After:
  global $bb_1 = funcref(optimized_trace_1_to_10)
```

**効果**:
- 呼び出し元のコードを一切変更せずに最適化を適用
- 次回から自動的に最適化コードが実行される
- インクリメンタルな最適化が可能

---

## 4. データ表現

### 4.1 Wasm GC活用

**構造体とarrayの利用**:
```wasm
;; オブジェクト表現（例）
(type $obj (struct
  (field $tag i32)      ;; 型タグ
  (field $data anyref)  ;; データ本体
))

;; リスト表現
(type $cons (struct
  (field $car anyref)
  (field $cdr anyref)
))
```

### 4.2 Unboxing（スカラ置換）

**目的**: 可能な限りネイティブ型（i64, f64など）で値を扱う

**実装箇所**:
- IR processor での型推論
- SSA最適化での型特殊化
- BBV による型ベースの分岐

```rust
// LocalType定義（ir/src/lib.rs）
pub enum LocalType {
    Type(Type),           // 具体的な型
    FuncRef,              // 関数参照
    MutFuncRef,           // mutable funcref
    VariadicArgs,         // 可変長引数
    EntrypointTable,      // エントリポイントテーブル
}

pub enum Type {
    Val(ValType),         // スカラ値
    Obj,                  // オブジェクト（boxed）
}

pub enum ValType {
    Int,    // i64
    Float,  // f64
    Bool,   // i32
    Nil,
    Char,
}
```

---

## 5. 制御フロー処理

### 5.1 Relooper アルゴリズム

**目的**: 非構造化制御フローを構造化制御フロー（block/loop/br）に変換

**実装** (wasm_generator/relooper.rs):
```rust
pub fn reloop_cfg(
    cfg: &VecMap<BasicBlockId, BasicBlock>,
    entry_bb: BasicBlockId,
) -> Vec<Structured> {
    // 支配木の構築
    let dom_tree = build_dom_tree(cfg, entry_bb);
    // Relooperアルゴリズムの適用
    let translator = Translator { ... };
    translator.do_tree(&dom_tree, &[])
}
```

**構造化された制御フロー**:
```rust
pub enum Structured {
    Simple(BasicBlockId),                    // 単純なBB
    Multiple(Vec<Structured>),               // 連続
    Loop { body: Vec<Structured> },          // ループ
    If { 
        cond_bb: BasicBlockId,
        then_body: Vec<Structured>,
        else_body: Vec<Structured>,
    },
    Break(usize),                            // break命令（相対インデックス）
    Exit(ExitInstr),                         // 関数終了
}
```

---

## 6. 最適化パイプライン

### 6.1 コンパイル時最適化（AOT）

```rust
// compiler.rs
fn optimize_module(module: &mut ir::Module, config: SsaOptimizerConfig) {
    let mut module_inliner = ModuleInliner::new(module);
    let n = 5;
    for i in 0..n {
        if config.enable_inlining {
            inlining(module, &mut module_inliner, i == n - 1);
        }
        for func in module.funcs.values_mut() {
            ssa_optimize(func, SsaOptimizerConfig {
                iterations: 1,
                ..config
            });
        }
    }
}
```

### 6.2 JIT時最適化（Tier 2）

**適用される最適化**:
- **定数伝搬**: 定数値の伝搬
- **デッドコード除去**: 到達不能コードの削除
- **共通部分式除去**: 重複計算の削減
- **インライン化**: 関数呼び出しの展開
- **型特殊化**: 型情報に基づく特殊化
- **スカラ置換**: オブジェクトのアンボックス化

```rust
// ssa_optimizer.rs
pub fn ssa_optimize(func: &mut Func, config: SsaOptimizerConfig) {
    for _ in 0..config.iterations {
        // デッドコード除去
        if config.enable_dce {
            eliminate_dead_code(func);
        }
        // 定数伝搬
        constant_propagation(func);
        // 共通部分式除去
        eliminate_common_subexpressions(func);
        // ...
    }
}
```

---

## 7. メモリ管理とランタイム

### 7.1 Wasm GCベースのメモリ管理

**特徴**:
- ガベージコレクションはWasmエンジンに委譲
- 明示的なメモリ管理は不要
- 構造体・配列の直接操作が可能

### 7.2 ランタイムAPI

**主要な関数** (webschembly-runtime-rust/src/lib.rs):

```rust
// JITコンパイルAPI
pub extern "C" fn instantiate_func(
    module_id: i32, 
    func_id: i32, 
    func_index: i32
) -> i32;

pub extern "C" fn instantiate_bb(
    module_id: i32,
    func_id: i32,
    func_index: i32,
    bb_id: i32,
    index: i32,
) -> i32;

// プロファイリングAPI
pub extern "C" fn increment_branch_counter(
    module_id: i32,
    func_id: i32,
    func_index: i32,
    bb_id: i32,
    kind: i32,
    source_bb_id: i32,
    source_index: i32,
);

// I/O API
pub extern "C" fn write_buf(fd: i32, buf_ptr: i32, buf_len: i32);
pub extern "C" fn flush_all();

// 型変換API
pub extern "C" fn _int_to_string(i: i64) -> i64;
pub extern "C" fn _float_to_string(f: f64) -> i64;
```

---

## 8. ベンチマーク・パフォーマンス分析

### 8.1 現在の性能

**Tier 2実装後の結果**:
- 一部ベンチマークでAOTコンパイラを超える性能を達成
- 全体平均ではAOTの約1.5倍遅い

### 8.2 ボトルネック分析

**解決済み**:
- ✅ 制御フローのオーバーヘッド → Tier 2線形化で解決

**残存する課題**:
- ❌ データアクセスのコスト
  - オブジェクトフィールドアクセス
  - 配列アクセス
  - 型チェック・型変換
- ❌ 関数呼び出しのコスト
  - 高階関数のオーバーヘッド
  - クロージャ生成
  - 間接呼び出し
- ❌ 動的言語機能のコスト
  - 継続（未実装）
  - 動的型システム

---

## 9. 今後

### 9.1 要求される追加最適化

**線形化JITに加えて、もう一つの強力な最適化手法**

### 9.2 候補となる最適化手法

1. **Polymorphic Inline Caching (PIC)**
   - 呼び出しサイトでの型情報キャッシュ
   - 高階関数呼び出しの高速化

2. **Escape Analysis & Stack Allocation**
   - オブジェクトのエスケープ解析
   - ヒープ割り当ての削減

3. **Speculative Optimization**
   - 仮定に基づく最適化
   - 仮定が破れた場合のdeoptimization

4. **Type Feedback & Adaptive Compilation**
   - ランタイム型情報の収集
   - 型プロファイルに基づく再コンパイル

5. **Object Shape Analysis**
   - オブジェクトの形状（フィールド配置）の推論
   - Hidden Classの導入

6. **Partial Evaluation**
   - 部分評価によるコード特殊化
   - メタプログラミング的な最適化

---

## 10. 開発ガイドライン

### 10.1 新機能の追加手順

1. **IR拡張**: 必要に応じて `webschembly-compiler-ir` にIR命令を追加
2. **JITロジック**: `jit/` 配下に新しいロジックを実装
3. **ランタイムサポート**: `webschembly-runtime-rust` にAPI追加
4. **テスト**: `webschembly-js/fixtures/` にテストケース追加

### 10.2 デバッグ方法

**IRダンプ**:
```rust
// debug_assertionsが有効な場合、IRが出力される
let ir = format!("{}", module.display());
```

**Wasmダンプ**:
```bash
# Wasmバイナリをテキスト形式に変換
wasm-objdump -d output.wasm
```

**ログ出力**:
```rust
log::debug!("branch specialize: module_id={}, func_id={}, ...", ...);
```

### 10.3 ベンチマーク実行

```bash
cd webschembly-js
just benchmark
```

---

## 11. 参考文献・関連技術

- **Relooper**: WebAssemblyへの構造化制御フロー変換アルゴリズム
- **SSA (Static Single Assignment)**: 静的単一代入形式
- **BBV (Basic Block Versioning)**: 基本ブロックバージョニング
- **Trace-based JIT**: トレースベースJITコンパイル
- **Wasm GC**: WebAssembly Garbage Collection proposal

---

## 付録A: グローバル変数レイアウト

**global_layout.rs** で管理される構造:

```rust
pub struct ClosureGlobalLayout {
    // クロージャインデックス → 引数型情報のマッピング
    // グローバル変数の割り当てを管理
}

pub const GLOBAL_LAYOUT_MAX_SIZE: usize = 256;
pub const GLOBAL_LAYOUT_DEFAULT_INDEX: usize = 0;
```

---

## 付録B: IR命令セット

主要なIR命令（`webschembly-compiler-ir/src/lib.rs`）:

```rust
pub enum InstrKind {
    // 値操作
    FromObj(LocalId, ValType),
    ToObj(LocalId, ValType),
    
    // 関数呼び出し
    Call(InstrCall),
    CallRef(InstrCallRef),
    CallClosure(InstrCallClosure),
    
    // グローバル変数操作
    GlobalGet(GlobalId),
    GlobalSet(GlobalId, LocalId),
    FuncRef(FuncId),
    
    // JIT特有
    InstantiateFunc(JitModuleId, FuncId, usize),
    IncrementBranchCounter(...),
    CreateMutFuncRef(LocalId),
    SetMutFuncRef(LocalId, LocalId),
    
    // SSA
    Phi { incomings: Vec<PhiIncoming> },
    
    // Terminator
    Terminator(TerminatorInstr),
}
```

---

このドキュメントは、AIが自立して開発を進めるために必要な情報を網羅しています。
詳細な実装やアルゴリズムについては、各ソースファイルのコメントも参照してください。
