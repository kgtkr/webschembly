# Webschembly ドキュメント索引

このディレクトリには、Webschemblyプロジェクトの技術ドキュメントが含まれています。

## 📚 ドキュメント一覧

### 1. [ARCHITECTURE.md](./ARCHITECTURE.md) - システムアーキテクチャ詳細

**対象読者**: 開発者、研究者、コントリビューター

**内容**:
- プロジェクト全体の概要と目標
- システムアーキテクチャの詳細図
- 2段階Tieringシステム（Tier 1 & Tier 2）の設計
- Global Dispatch機構の仕組み
- Basic Block Versioning (BBV)
- Trace Linearization（トレース線形化）
- データ表現とWasm GC活用
- 制御フロー処理（Relooper）
- 最適化パイプライン
- メモリ管理とランタイムAPI
- IR命令セット

**こんな時に読む**:
- システム全体の構造を理解したい
- JITコンパイラの動作原理を知りたい
- 既存の最適化手法を学びたい
- 新しい機能を追加する前に全体像を把握したい

### 2. [DEVELOPMENT_GUIDE.md](./DEVELOPMENT_GUIDE.md) - 開発ガイド

**対象読者**: 新規コントリビューター、開発者

**内容**:
- 開発環境のセットアップ手順
- コードベースの構造と各モジュールの役割
- 開発ワークフロー（ビルド、テスト、デプロイ）
- 実装例: Polymorphic Inline Cachingの追加
- テストとデバッグの方法
- ベンチマークの実行と追加
- トラブルシューティング
- リリースプロセス

**こんな時に読む**:
- 初めてコードに触る
- 新しい機能を実装したい
- テストやデバッグの方法を知りたい
- ベンチマークを実行したい
- エラーに遭遇した

### 3. [RESEARCH_STRATEGY.md](./RESEARCH_STRATEGY.md) - 研究戦略と次期最適化提案

**対象読者**: 研究者、論文執筆者

**内容**:
- 現状の成果と残存課題の分析
- 性能ボトルネックの詳細分析
- 候補となる最適化手法の評価
  - Polymorphic Inline Caching (PIC) ⭐ 推奨
  - Escape Analysis & Stack Allocation
  - Speculative Optimization
  - Type Feedback & Adaptive Compilation
  - Object Shape Analysis
  - Partial Evaluation
- 推奨アプローチと実装戦略
- 実装ロードマップ（週単位）
- 論文構成案
- リスク管理

**こんな時に読む**:
- 次に何を実装すべきか決めたい
- 論文の構成を考えている
- 各最適化手法のメリット・デメリットを比較したい
- 実装スケジュールを立てたい

### 4. [r5rs.md](./r5rs.md) - R5RS準拠状況

**対象読者**: ユーザー、言語仕様に関心のある開発者

**内容**:
- R5RS標準仕様との差異
- 未実装機能（継続など）
- 実装上の制約と理由

**こんな時に読む**:
- Schemeコードの互換性を確認したい
- 利用可能な機能を知りたい

### 5. [function_jit.md](./function_jit.md) - JIT詳細（図表含む）

**対象読者**: 論文執筆者、詳細な実装を知りたい開発者

**内容**:
- JITコンパイルの可視化
- 図表（LaTeX形式）

---

## 🎯 目的別ガイド

### 初めてプロジェクトに触る場合

1. **README.md** - プロジェクト概要とクイックスタート
2. **ARCHITECTURE.md** - システム全体の理解
3. **DEVELOPMENT_GUIDE.md** - 開発環境のセットアップ

### 新機能を実装したい場合

1. **ARCHITECTURE.md** - 既存システムの理解
2. **DEVELOPMENT_GUIDE.md** § 4 - 実装例を参考に
3. **RESEARCH_STRATEGY.md** - 最適化の選定

### 論文を書く場合

1. **ARCHITECTURE.md** - 技術的背景
2. **RESEARCH_STRATEGY.md** - 研究戦略と評価計画
3. **RESEARCH_STRATEGY.md** § 6 - 論文構成案

### トラブルシューティング

1. **DEVELOPMENT_GUIDE.md** § 7 - トラブルシューティング
2. **ARCHITECTURE.md** - 仕組みの理解
3. GitHub Issues - 既知の問題

---

## 📊 ドキュメントマップ

```
README.md (入口)
    ↓
    ├─→ ARCHITECTURE.md (What & How)
    │       ↓
    │       ├─ システム設計
    │       ├─ JIT実装詳細
    │       └─ IR命令セット
    │
    ├─→ DEVELOPMENT_GUIDE.md (How to)
    │       ↓
    │       ├─ セットアップ
    │       ├─ 実装例
    │       ├─ テスト/デバッグ
    │       └─ トラブルシューティング
    │
    └─→ RESEARCH_STRATEGY.md (What's Next)
            ↓
            ├─ 現状分析
            ├─ 最適化提案
            ├─ ロードマップ
            └─ 論文構成
```

---

## 🔄 ドキュメントの更新

ドキュメントは以下の場合に更新してください:

- **ARCHITECTURE.md**: 
  - 新しい主要機能の追加
  - アーキテクチャの変更
  - IR命令の追加/変更

- **DEVELOPMENT_GUIDE.md**: 
  - 開発プロセスの変更
  - 新しいツールの導入
  - よくあるエラーの発見

- **RESEARCH_STRATEGY.md**: 
  - 新しい最適化手法の評価
  - ベンチマーク結果の更新
  - 研究方向の変更

---

## 📝 ドキュメント執筆ガイドライン

1. **明確さ**: 技術用語は初出時に説明
2. **完全性**: コード例は動作するものを掲載
3. **最新性**: コード変更時はドキュメントも更新
4. **構造**: 目次とセクション番号を使用
5. **クロスリファレンス**: 関連ドキュメントへのリンク

---

## 🆘 サポート

- **バグ報告**: [GitHub Issues](https://github.com/kgtkr/webschembly/issues)
- **質問**: Discussionsまたはメール
- **貢献**: Pull Requestを歓迎します

---

最終更新: 2026年1月
