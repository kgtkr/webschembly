# Webschembly

Scheme JIT compiler for WebAssembly

- [Playground](https://kgtkr.github.io/webschembly/)
- [Benchmark](https://kgtkr.github.io/webschembly/dev/bench)

## Documentation

**For Developers & Researchers:**
- 📘 [Architecture Documentation](./docs/ARCHITECTURE.md) - Complete system architecture and JIT implementation details
- 🛠️ [Development Guide](./docs/DEVELOPMENT_GUIDE.md) - Development workflow, implementation examples, and debugging tips
- 🔬 [Research Strategy](./docs/RESEARCH_STRATEGY.md) - Performance analysis, optimization proposals, and research roadmap

**For Users:**
- [R5RS Compliance](./docs/r5rs.md) - Differences from R5RS standard

## Quick Start

### Prerequisites

- Nix
- Direnv

### Setup

```bash
# Clone the repository
git clone https://github.com/kgtkr/webschembly.git
cd webschembly

# Enable direnv
direnv allow

# Build the project
cargo build --all
```

### Usage

```bash
# Run Scheme code
cd webschembly-js
npm install
npm run build
node dist/cli.js your-program.scm

# Run REPL
node dist/repl.js
```

## Project Structure

This project consists of the following components:

- webschembly-compiler: Scheme → WebAssembly compiler library
- webschembly-compiler-cli: Command-line interface for the compiler, mainly used for debugging generated code
- webschembly-js: JavaScript bindings with CLI execution and REPL capabilities
- webschembly-playground: Web-based playground
- webschembly-runtime: Runtime library
- webschembly-runtime-rust: Rust-implemented parts of the runtime library

## Requirements

- Nix
- Direnv

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
