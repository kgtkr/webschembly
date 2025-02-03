SCM := $(wildcard src/*.scm)
LIST := $(patsubst %.scm,%,$(SCM))
WASM := $(patsubst %.scm,%.wasm,$(SCM))
OBJDUMP := $(patsubst %.scm,%.objdump,$(SCM))
RESULT := $(patsubst %.scm,%.result,$(SCM))
WAT := $(patsubst %.scm,%.wat,$(SCM))
SNAPSHOT := $(patsubst %.scm,%.snapshot,$(SCM))

.PHONY: compiler
compiler:
	cd ../ && cargo build --target wasm32-unknown-unknown && cargo build

%.wasm: %.scm compiler
	../target/debug/webschembly-compiler-cli $< $@

%.objdump: %.wasm
	wasm-objdump -hx $< > $@

%.result: %.wasm
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
