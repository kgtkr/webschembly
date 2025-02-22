{ stdenv, binaryen }:
stdenv.mkDerivation {
  name = "webschembly-runtime-wat";
  src = ./.;
  buildInputs = [ binaryen ];
  buildPhase = ''
    wasm-as -o webschembly_runtime_wat.wasm $src/lib.wat
  '';
  installPhase = ''
    mkdir -p $out/lib
    cp webschembly_runtime_wat.wasm $out/lib
  '';
}
