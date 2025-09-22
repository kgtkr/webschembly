BINARYEN_ARGS ?= $(shell cat ../binaryen-args.txt)
CARGO_WEBSCHEMBLY_RUNTIME_RUST_DEBUG := ../target/wasm32-unknown-unknown/debug/webschembly_runtime_rust.wasm
CARGO_WEBSCHEMBLY_RUNTIME_RUST_RELEASE := ../target/wasm32-unknown-unknown/release/webschembly_runtime_rust.wasm

ifeq ($(CARGO_RELEASE), 1)
CARGO_WEBSCHEMBLY_RUNTIME_RUST := $(CARGO_WEBSCHEMBLY_RUNTIME_RUST_RELEASE)
else
CARGO_WEBSCHEMBLY_RUNTIME_RUST := $(CARGO_WEBSCHEMBLY_RUNTIME_RUST_DEBUG)
endif

WEBSCHEMBLY_RUNTIME_RUST ?= $(CARGO_WEBSCHEMBLY_RUNTIME_RUST)

.PHONY: build
build: webschembly_runtime.wasm

webschembly_runtime.wasm: webschembly_runtime_wat.wasm $(WEBSCHEMBLY_RUNTIME_RUST)
	wasm-merge -o $@ $(foreach f, $^, $(f) runtime) $(BINARYEN_ARGS)

webschembly_runtime_wat.wasm: lib.wat
	wasm-as $(BINARYEN_ARGS) -o $@ $<

.PHONY: $(CARGO_WEBSCHEMBLY_RUNTIME_RUST_DEBUG)
$(CARGO_WEBSCHEMBLY_RUNTIME_RUST_DEBUG):
	cd ../ && cargo build --target wasm32-unknown-unknown

.PHONY: $(CARGO_WEBSCHEMBLY_RUNTIME_RUST_RELEASE)
$(CARGO_WEBSCHEMBLY_RUNTIME_RUST_RELEASE):
	cd ../ && cargo build --target wasm32-unknown-unknown --release


.PHONY: clean
clean:
	rm -f *.wasm
