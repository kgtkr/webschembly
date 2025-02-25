{ stdenv, binaryen, webschembly-runtime-rust, gnumake, BINARYEN_ARGS }:
stdenv.mkDerivation {
  name = "webschembly-runtime";
  src = ./.;
  buildInputs = [ binaryen webschembly-runtime-rust gnumake ];
  buildPhase = ''
    make WEBSCHEMBLY_RUNTIME_RUST=${webschembly-runtime-rust}/lib/webschembly_runtime_rust.wasm BINARYEN_ARGS="${BINARYEN_ARGS}"
  '';
  installPhase = ''
    mkdir -p $out/lib
    cp webschembly_runtime.wasm $out/lib
  '';
}
