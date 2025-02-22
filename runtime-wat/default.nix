{ stdenv, binaryen }:
stdenv.mkDerivation {
  name = "runtime-wat";
  src = ./.;
  buildInputs = [ binaryen ];
  buildPhase = ''
    wasm-as -o runtime.wasm $src/runtime.wat
  '';
  installPhase = ''
    mkdir -p $out/lib
    cp runtime.wasm $out/lib
  '';
}
