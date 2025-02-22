{ stdenv, binaryen, runtime-rust, runtime-wat }:
stdenv.mkDerivation {
  name = "runtime";
  dontUnpack = true;
  buildInputs = [ binaryen runtime-rust runtime-wat ];
  buildPhase = ''
    wasm-merge -o runtime.wasm ${runtime-rust}/lib/webschembly_runtime.wasm runtime ${runtime-wat}/lib/runtime.wasm runtime --enable-reference-types --enable-multimemory
  '';
  installPhase = ''
    mkdir -p $out/lib
    cp runtime.wasm $out/lib
  '';
}
