SCM := $(wildcard src/*.scm)
LIST := $(patsubst %.scm,%,$(SCM))
WASM := $(patsubst %.scm,%.wasm,$(SCM))
WASMS := $(patsubst %.scm,%.wasms,$(SCM))
OBJDUMP := $(patsubst %.scm,%.objdump,$(SCM))
RESULT := $(patsubst %.scm,%.result,$(SCM))
WAT := $(patsubst %.scm,%.wat,$(SCM))
SNAPSHOT := $(patsubst %.scm,%.snapshot,$(SCM))

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

%.wasm: %.wasms $(WEBSCHEMBLY_RUNTIME)
	wasm-merge -o $@ $(WEBSCHEMBLY_RUNTIME) runtime $(foreach f, $(wildcard $</*.wasm), $(f) dummy_input) --enable-reference-types --enable-multimemory

# TODO:
# wasm-objdumpはGC proposalに対応していないのでそれを含むwasmファイルを与えるとエラーになる
# しかし、wasm-dis は型エラーがあるwasmファイルを与えるとエラーになるのでデバッグには使いにくい
%.objdump: %.wasm
	wasm-objdump -hxd $< > $@

%.result: %.scm $(WEBSCHEMBLY_RUNTIME)
	mkdir -p $@
	rm -f $@/*
	WEBSCHEMBLY_RUNTIME=$(WEBSCHEMBLY_RUNTIME) node run.js $< > $@/stdout 2> $@/stderr; echo $$? > $@/exitcode

%.wat: %.wasm
	wasm-dis $< -o $@

%.snapshot: %.result
	mkdir -p $@
	rm -rf $(wildcard $@/*)
	cp $(wildcard $</*) $@

.PHONY: generate-snapshot
generate-snapshot: $(SNAPSHOT)

.PHONY: test
test: $(RESULT)
	$(SHELL) ./test.sh $(LIST)

.PHONY: clean
clean:
	rm -f $(WASM) $(OBJDUMP) $(WAT)
	rm -rf $(RESULT) $(WASMS)

.PHONY: all
all: $(WASM) $(OBJDUMP) $(RESULT) $(WAT)
