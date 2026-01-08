# Architecture Documentation

This document describes the architecture of the **WebSchembly** compiler, a dynamic JIT compiler for R5RS Scheme targeting WebAssembly (Wasm). It details the execution model, compilation tiers, data representation, and code structure to assist in autonomous development and maintenance.

## 1. Project Overview

**Goal**: Achieve high-performance execution of R5RS Scheme on WebAssembly by implementing a dynamic JIT compiler with advanced optimizations comparable to AOT compilers.
**Key Feature**: A multi-tier JIT architecture leveraging WebAssembly's global variables for dynamic dispatch and trace-based optimization.

## 2. Execution Model & JIT Architecture

The system employs a **Global Dispatch Table** mechanism using Wasm `mut funcref` global variables to realize dynamic linking and hot-code swapping.

### Global Dispatch Table

- **Concept**: Each basic block (BB) or function entry point is associated with a Wasm global variable (`mut funcref`).
- **Mechanism**: Transitions between blocks are implemented by reading the target function reference from the corresponding global variable and executing `return_call_ref` (tail call).
- **Benefit**: Allows the JIT compiler to update the code of a specific block/function dynamically by simply updating the global variable, without recompiling the caller.

### Tier 1: Baseline JIT

The first tier provides fast compilation times and enables adaptive optimization.

- **Basic Block (BB) Compilation**: Each logical Basic Block is compiled into a separate Wasm function.
- **Lazy Compilation (Stubs)**:
  - Initially, global variables point to "Stub Functions".
  - When a stub is executed, it triggers the JIT compiler to generate the actual code for the block.
  - The global variable is then updated to point to the generated code.
- **Basic Block Versioning (BBV)**:
  - To handle dynamic typing efficiently, multiple versions of a BB are generated based on the types of incoming arguments and local variables.
  - **Implementation**: Managed by `BBIndexManager` and `EnvIndexManager` in `src/jit/global_layout.rs`.
  - **Limit**: A fixed limit (`GLOBAL_LAYOUT_MAX_SIZE`, e.g., 32) prevents code explosion due to polymorphism.

### Tier 2: Trace Linearization (Optimization)

The second tier optimizes hot paths to reduce control flow overhead.

- **Profiling**:
  - Counters are embedded at branch points in Tier 1 code (`start_branch_profiling` / `increment_branch_counter`).
  - **Hot Path Detection**: When execution count exceeds a threshold, a "trace" (sequence of frequently executed blocks) is identified.
- **Trace Linearization**:
  - Adjacent BBs in the hot path are inlined and combined into a single, larger Wasm function.
  - **Relooper**: The `relooper` algorithm (in `src/wasm_generator/relooper.rs`) reconstructs structured control flow (Wasm `block`/`loop`) from the linear trace, replacing expensive `call_ref` chains with efficient internal jumps.
- **Promotion**:
  - The entry point of the optimized path (e.g., BB1) in the Global Dispatch Table is updated to point to the new linearized function.
  - Callers automatically switch to the optimized version without modification.

## 3. Data Representation

- **Memory**: Utilizes Wasm GC features (`struct` and `array` types) for object management, delegation garbage collection to the host VM.
- **Value Representation**:
  - **Unboxing**: Values are scalar-replaced where possible and treated as native Wasm types (e.g., `i64`) to minimize allocation overhead.
  - **Boxing**: When necessary, values are boxed into GC-managed structs.

## 4. Source Code Structure

The core logic resides in `webschembly-compiler`:

### `src/jit`

Contains the JIT compilation logic and state management.

- **`mod.rs`**: Entry point `Jit`, exposes `increment_branch_counter` for profiling.
- **`jit_module.rs`**: Manages JIT modules (`JitModule`), handles stub generation (`generate_stub_module`), and orchestrates function instantiation.
- **`jit_func.rs`**: Core logic for compiling functions and Basic Blocks (`JitFunc`, `JitSpecializedArgFunc`). Handles the specialization logic for BBV.
- **`global_layout.rs`**: Manages the mapping between type configurations (argument types) and Global Dispatch Indices (`BBIndexManager`, `EnvIndexManager`).
- **`jit_ctx.rs`**: JIT context and configuration.

### `src/wasm_generator`

Handles the generation of final WebAssembly binary/text.

- **`module_generator.rs`**: Generates Wasm modules.
- **`relooper.rs`**: Implements the Relooper algorithm to recover structured control flow from the CFG, crucial for Tier 2 optimization.

### `src/ir_processor`

Intermediate Representation (IR) processing and static analysis.

- **`register_allocation.rs`**: Register allocation logic (Linear Scan).
- **`ssa.rs`**: SSA construction.

## 5. Known Challenges & Future Work

- **Performance gap**: Initial Tier 2 implementation is ~1.5x slower than AOT.
  - **Bottlenecks**: Data access, high-order function calls, and control features (continuations).
- **Objective**: Implement further optimizations "beyond linearization" to close the performance gap.
