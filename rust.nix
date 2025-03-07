{ lib, inputs, ... }: {
  perSystem = { config, system, pkgs, cargo2nix-ifd-lib, ... }:
    let
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      projectName = "webschembly";
      staticTarget = pkgs.pkgsStatic.stdenv.hostPlatform.config;
      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions ([
          ./Cargo.toml
          ./Cargo.lock
        ] ++ lib.map (lib.path.append ./.) (fromTOML (builtins.readFile ./Cargo.toml)).workspace.members);
      };
      filteredSrc = cargo2nix-ifd-lib.filterSrc {
        inherit src;
        orFilter = path: _type:
          let
            files = [ "stdlib.scm" ];
          in
          lib.any (file: baseNameOf path == file) files;
        inherit projectName;
      };
      generatedSrc = cargo2nix-ifd-lib.generateSrc {
        src = filteredSrc;
        inherit projectName rustToolchain;
      };
      staticRustPkgs = pkgs.rustBuilder.makePackageSet {
        packageFun = import "${generatedSrc}/Cargo.nix";
        target = staticTarget;
        rustToolchain = rustToolchain.override {
          targets = [ staticTarget ];
        };
      };
      wasmRustPkgs = pkgs.rustBuilder.makePackageSet {
        packageFun = import "${generatedSrc}/Cargo.nix";
        target = "wasm32-unknown-unknown";
        inherit rustToolchain;
      };
      webschembly-compiler-cli = (staticRustPkgs.workspace.webschembly-compiler-cli { }).bin;
      webschembly-runtime-rust = (wasmRustPkgs.workspace.webschembly-runtime-rust { }).out;
      webschembly-runtime = pkgs.callPackage ./webschembly-runtime { inherit webschembly-runtime-rust; BINARYEN_ARGS = lib.strings.trim (builtins.readFile ./binaryen-args.txt); };
    in
    {
      packages = {
        inherit webschembly-compiler-cli webschembly-runtime;
      };
      make-shells.default = {
        env = {
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
        packages = [
          rustToolchain
          pkgs.cargo-insta
        ];
      };
    };
}
