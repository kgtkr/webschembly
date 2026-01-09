# Debugging Guide

This document outlines the workflow and tools for debugging the Webschembly compiler and JIT.

## Running Tests with Logs

To debug specific Scheme files or tests, use `just run` within the `webschembly-js` directory.

### Environment Variables

- `LOG_STDOUT=1`: Enables `log::debug!` output from the runtime and compiler (if configured). Use this to see runtime execution trace, JIT instantiation events, and error messages.
 - `println!` output is not visible here. Please use `log::debug!` instead.
- `LOG=1`: Dumps the generated Intermediate Representation (IR) and Wasm binaries to the `webschembly-js/log/` directory.

### Command Examples

```bash
# Run a specific fixture with runtime logs
cd webschembly-js
just LOG_STDOUT=1 run ./fixtures/rec.scm

# Run with IR dumping
just LOG=1 run ./fixtures/rec.scm
```

## Analyzing Generated IR

When `LOG=1` is used, the `webschembly-js/log/` directory will contain files named with the timestamp and content description.

- `checks-TIMESTAMP-filename-0.ir`: Typically the main module IR (initial compilation).
- `checks-TIMESTAMP-filename-N.ir`: IR for JIT-compiled functions or stubs. `instantiate_func` or `instantiate_bb` calls in the runtime logs (seen with `LOG_STDOUT=1`) will reference `module_id` and `func_id`, which correlate to these files (though the mapping requires checking the `instantiate` log id vs file index).

**Tip**: Look for "instantiate: id:X" in the runtime logs. The corresponding IR file is often suffix `-X.ir`.

## Common Issues & Fixes

### "call target is not a closure"

This error occurs when the compiled code attempts to invoke a value as a closure, but the compile-time or run-time check fails.

- **Runtime**: The value on the stack is not a closure object (e.g., encoded as `val_type` mismatch).
- **Compile-time (Optimization)**: If the error appears unconditionally in the IR (e.g., `error "call target is not a closure"`), it means `constant_folding` or another pass determined the check `Is(Closure(None), target)` is false.
  - _Watch out for_: Type mismatches in `InstrKind::Is`. Ensure `Closure(None)` (generic check) correctly matches specialized types like `Closure(Some(C))` (constant closure). Using `.remove_constant()` on types before comparison is crucial in `ssa_optimizer.rs`.

## JIT Optimization Logic

- **`propagate_types` Pass**: Analyzes dataflow to identify constant closures (`Closure(Some(C))`). Logic in `src/ir_processor/propagate_types.rs`.
- **Specialization**: `jit_func.rs` uses available type info (from `locals`) to unbox closure entries or arguments.
- **Constant Folding**: `ssa_optimizer.rs` folds constants. Be careful with strict equality checks on specialized types.