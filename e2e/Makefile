.PHONY: *.scm
*.scm:
	cd ../ && cargo build --target wasm32-unknown-unknown && cargo build
	../target/debug/webschembly-compiler-cli $@ $@.wasm
	wasm-objdump -hx $@.wasm
	node test.js $@.wasm

.PHONY: clean
clean:
	rm *.wasm
