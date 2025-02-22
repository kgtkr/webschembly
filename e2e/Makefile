SCM := $(wildcard src/*.scm)
LIST := $(patsubst %.scm,%,$(SCM))
WASM := $(patsubst %.scm,%.wasm,$(SCM))
WASMS := $(patsubst %.scm,%.wasms,$(SCM))
OBJDUMP := $(patsubst %.scm,%.objdump,$(SCM))
RESULT := $(patsubst %.scm,%.result,$(SCM))
WAT := $(patsubst %.scm,%.wat,$(SCM))
SNAPSHOT := $(patsubst %.scm,%.snapshot,$(SCM))

.PHONY: bin/cli
bin/cli:
	mkdir -p bin
	cd ../ && cargo build --bin webschembly-compiler-cli
	ln -sf $$PWD/../target/debug/webschembly-compiler-cli bin/cli

.PHONY: bin/runtime-rust.wasm
bin/runtime-rust.wasm:
	mkdir -p bin
	cd ../ && cargo build --target wasm32-unknown-unknown --lib
	ln -sf $$PWD/../target/wasm32-unknown-unknown/debug/webschembly_runtime.wasm $@

.PHONY: bin/runtime-wat.wasm
bin/runtime-wat.wasm:
	mkdir -p bin
	wasm-as ../runtime-wat/runtime.wat -o $@

bin/runtime.wasm: bin/runtime-rust.wasm bin/runtime-wat.wasm
	wasm-merge -o $@ $(foreach f, $^, $(f) runtime)

%.wasms: %.scm bin/cli
	mkdir -p $@
	rm -f $@/*.wasm
	bin/cli $< -o $@/o.wasm

%.wasm: %.wasms bin/runtime.wasm
	wasm-merge -o $@ bin/runtime.wasm runtime $(foreach f, $(wildcard $</*.wasm), $(f) dummy_input) --enable-reference-types --enable-multimemory

# TODO:
# wasm-objdumpはGC proposalに対応していないのでそれを含むwasmファイルを与えるとエラーになる
# しかし、wasm-dis は型エラーがあるwasmファイルを与えるとエラーになるのでデバッグには使いにくい
%.objdump: %.wasm
	wasm-objdump -hxd $< > $@

%.result: %.scm bin/runtime.wasm
	mkdir -p $@
	rm -f $@/*
	node run.js $< > $@/stdout 2> $@/stderr; echo $$? > $@/exitcode

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
