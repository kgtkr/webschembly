BINARYEN_ARGS ?= $(shell cat ../binaryen-args.txt)
SCM := $(wildcard fixtures/*.scm)
LIST := $(patsubst %.scm,%,$(SCM))
WASM := $(patsubst %.scm,%.wasm,$(SCM))
WASMS := $(patsubst %.scm,%.wasms,$(SCM))
WAT := $(patsubst %.scm,%.wat,$(SCM))

LOCAL_WEBSCHEMBLY_RUNTIME := ../webschembly-runtime/webschembly_runtime.wasm
WEBSCHEMBLY_RUNTIME ?= $(LOCAL_WEBSCHEMBLY_RUNTIME)

CARGO_WEBSCHEMBLY_COMPILER_CLI := ../target/debug/webschembly-compiler-cli
WEBSCHEMBLY_COMPILER_CLI ?= $(CARGO_WEBSCHEMBLY_COMPILER_CLI)


.PHONY: $(LOCAL_WEBSCHEMBLY_RUNTIME)
$(LOCAL_WEBSCHEMBLY_RUNTIME):
	$(MAKE) -C ../webschembly-runtime

.PHONY: $(WEBSCHEMBLY_COMPILER_CLI)
$(WEBSCHEMBLY_COMPILER_CLI):
	cd ../ && cargo build --bin webschembly-compiler-cli

%.wasms: %.scm $(WEBSCHEMBLY_COMPILER_CLI)
	mkdir -p $@
	rm -f $@/*.wasm
	$(WEBSCHEMBLY_COMPILER_CLI) $< -o $@/o.wasm

%.wats: %.wasms
	mkdir -p $@
	rm -f $@/*.wat
	# wasm-toolsのwasm -> watはバリデーションを行わないのでデバッグしやすい
	$(foreach f, $(wildcard $</*.wasm), wasm-tools print $(f) -o $@/$(notdir $(f:.wasm=.wat));)
	# watに変換してバリデーションしたほうが分かりやすいため
	$(foreach f, $(wildcard $</*.wasm), wasm-as $(BINARYEN_ARGS) $@/$(notdir $(f:.wasm=.wat));)


%.wasm: %.wasms $(WEBSCHEMBLY_RUNTIME)
	# TODO: --rename-export-conflictsはできればつけたくない
	wasm-merge -o $@ $(WEBSCHEMBLY_RUNTIME) runtime $(foreach f, $(wildcard $</*.wasm), $(f) dummy_input) $(BINARYEN_ARGS) --rename-export-conflicts


%.wat: %.wasm
	wasm-dis $< -o $@

.PHONY: clean
clean:
	rm -f $(WASM) $(WAT)
	rm -rf $(WASMS) $(WATS)

.PHONY: all
all: $(WASM) $(WAT)

benchmark.result: $(WEBSCHEMBLY_RUNTIME)
	WEBSCHEMBLY_RUNTIME=$(WEBSCHEMBLY_RUNTIME) npm run benchmark

.PHONY: benchmark
benchmark: benchmark.result
