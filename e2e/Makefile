SCM := $(wildcard src/*.scm)
LIST := $(patsubst %.scm,%,$(SCM))
WASM := $(patsubst %.scm,%.wasm,$(SCM))
OBJDUMP := $(patsubst %.scm,%.objdump,$(SCM))
RESULT := $(patsubst %.scm,%.result,$(SCM))
WAT := $(patsubst %.scm,%.wat,$(SCM))
SNAPSHOT := $(patsubst %.scm,%.snapshot,$(SCM))

.PHONY: bin/cli
bin/cli:
	mkdir -p bin
	cd ../ && cargo build --bin webschembly-compiler-cli
	ln -sf $$PWD/../target/debug/webschembly-compiler-cli bin/cli

.PHONY: bin/runtime.wasm
bin/runtime.wasm:
	mkdir -p bin
	cd ../ && cargo build --target wasm32-unknown-unknown --lib
	ln -sf $$PWD/../target/wasm32-unknown-unknown/debug/webschembly_runtime.wasm bin/runtime.wasm


%.wasm: %.scm bin/cli
	bin/cli $< $@

%.objdump: %.wasm
	wasm-objdump -hx $< > $@

%.result: %.wasm bin/runtime.wasm
	mkdir -p $@
	rm -f $@/*
	node run.js $< > $@/stdout 2> $@/stderr; echo $$? > $@/exitcode

%.wat: %.wasm
	wasm2wat $< -o $@

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
	rm -rf $(RESULT)

.PHONY: all
all: $(WASM) $(OBJDUMP) $(RESULT) $(WAT)
