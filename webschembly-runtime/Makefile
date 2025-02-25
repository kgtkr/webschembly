BINARYEN_ARGS ?= $(shell cat ../binaryen-args.txt)
CARGO_WEBSCHEMBLY_RUNTIME_RUST := ../target/wasm32-unknown-unknown/debug/webschembly_runtime_rust.wasm
WEBSCHEMBLY_RUNTIME_RUST ?= $(CARGO_WEBSCHEMBLY_RUNTIME_RUST)

.PHONY: build
build: webschembly_runtime.wasm

webschembly_runtime.wasm: webschembly_runtime_wat.wasm $(WEBSCHEMBLY_RUNTIME_RUST)
	wasm-merge -o $@ $(foreach f, $^, $(f) runtime) $(BINARYEN_ARGS)

webschembly_runtime_wat.wasm: lib.wat
	wasm-as $(BINARYEN_ARGS) -o $@ $<

.PHONY: $(CARGO_WEBSCHEMBLY_RUNTIME_RUST)
$(CARGO_WEBSCHEMBLY_RUNTIME_RUST):
	cd ../ && cargo build --target wasm32-unknown-unknown

.PHONY: clean
clean:
	rm -f *.wasm
