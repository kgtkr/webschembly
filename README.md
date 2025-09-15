# Webschembly

Scheme JIT compiler for WebAssembly

* [Playground](https://kgtkr.github.io/webschembly/)
* [Benchmark](https://kgtkr.github.io/webschembly/dev/bench)

## Project Structure
This project consists of the following components:

* webschembly-compiler: Scheme → WebAssembly compiler library
* webschembly-compiler-cli: Command-line interface for the compiler, mainly used for debugging generated code
* webschembly-js: JavaScript bindings with CLI execution and REPL capabilities
* webschembly-playground: Web-based playground
* webschembly-runtime: Runtime library
* webschembly-runtime-rust: Rust-implemented parts of the runtime library

## Requirements
* Nix
* Direnv

Run `direnv allow` to set up the development environment.

## Sample Code

```scheme
;; Simple addition
(write (+ 1 2))
(newline)

;; Function definition
(define (factorial n)
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))

(write (factorial 5))
(newline)
```
