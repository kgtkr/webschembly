# Webschembly

Scheme JIT compiler for WebAssembly

[Playground](https://kgtkr.github.io/webschembly/)

## プロジェクト構成
このプロジェクトは以下のコンポーネントで構成されています：

* webschembly-compiler: Scheme → WebAssemblyコンパイラライブラリ
* webschembly-compiler-cli: コンパイラのコマンドラインインターフェース。主に生成コードのデバッグのために使う
* webschembly-js: JavaScriptバインディング。Cliでの実行やREPLが利用可能
* webschembly-playground: Webベースのプレイグラウンド
* webschembly-runtime: ランタイムライブラリ
* webschembly-runtime-rust: ランタイムライブラリのうちRustで実装されている部分

## Requirements
* Nix
* Direnv

`direnv allow` を実行すると開発環境がセットアップされます。

## サンプルコード

```scheme
;; 単純な加算
(write (+ 1 2))
(newline)

;; 関数定義
(define (factorial n)
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))

(write (factorial 5))
(newline)
```
