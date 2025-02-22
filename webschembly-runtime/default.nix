{ stdenv, binaryen, webschembly-runtime-rust, webschembly-runtime-wat }:
stdenv.mkDerivation {
  name = "webschembly-runtime";
  dontUnpack = true;
  buildInputs = [ binaryen webschembly-runtime-rust webschembly-runtime-wat ];
  buildPhase = ''
    wasm-merge -o webschembly_runtime.wasm ${webschembly-runtime-rust}/lib/webschembly_runtime_rust.wasm runtime ${webschembly-runtime-wat}/lib/webschembly_runtime_wat.wasm runtime --enable-reference-types --enable-multimemory
  '';
  installPhase = ''
    mkdir -p $out/lib
    cp webschembly_runtime.wasm $out/lib
  '';
}
