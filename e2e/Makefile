.PHONY: test
test:
	cd ../ && cargo build --target wasm32-unknown-unknown && cargo build
	../target/debug/webschembly-compiler-cli test.scm test.wasm
	node test.js
