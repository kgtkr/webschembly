SCM := $(wildcard src/*.scm)
LIST := $(patsubst %.scm,%,$(SCM))
WASM := $(patsubst %.scm,%.wasm,$(SCM))
WASMS := $(patsubst %.scm,%.wasms,$(SCM))
OBJDUMP := $(patsubst %.scm,%.objdump,$(SCM))
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
	$(foreach f, $(wildcard $</*.wasm), wasm-dis $(f) -o $@/$(notdir $(f:.wasm=.wat));)


%.wasm: %.wasms $(WEBSCHEMBLY_RUNTIME)
	wasm-merge -o $@ $(WEBSCHEMBLY_RUNTIME) runtime $(foreach f, $(wildcard $</*.wasm), $(f) dummy_input) --enable-reference-types --enable-multimemory

# TODO:
# wasm-objdumpはGC proposalに対応していないのでそれを含むwasmファイルを与えるとエラーになる
# しかし、wasm-dis は型エラーがあるwasmファイルを与えるとエラーになるのでデバッグには使いにくい
%.objdump: %.wasm
	wasm-objdump -hxd $< > $@

%.wat: %.wasm
	wasm-dis $< -o $@

.PHONY: test
test: $(WEBSCHEMBLY_RUNTIME)
	WEBSCHEMBLY_RUNTIME=$(WEBSCHEMBLY_RUNTIME) npm test

.PHONY: repl
repl: $(WEBSCHEMBLY_RUNTIME)
	WEBSCHEMBLY_RUNTIME=$(WEBSCHEMBLY_RUNTIME) node repl.js

.PHONY: run
run: $(WEBSCHEMBLY_RUNTIME)
	WEBSCHEMBLY_RUNTIME=$(WEBSCHEMBLY_RUNTIME) node run.js $(SRC)

.PHONY: clean
clean:
	rm -f $(WASM) $(OBJDUMP) $(WAT)
	rm -rf $(WASMS)

.PHONY: all
all: $(WASM) $(OBJDUMP) $(WAT)
