{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      url = "github:kgtkr/cargo2nix/396edea";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    cargo2nix-ifd = {
      url = "github:kgtkr/cargo2nix-ifd";
      inputs.cargo2nix.follows = "cargo2nix";
    };
  };

  outputs = { self, nixpkgs, flake-utils, cargo2nix, cargo2nix-ifd, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ cargo2nix.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        projectName = "webschembly";
        staticTarget = pkgs.pkgsStatic.stdenv.hostPlatform.config;
        filteredSrc = cargo2nix-ifd.lib.${system}.filterSrc {
          src = ./.;
          orFilter = orig_path: type:
            let
              path = (toString orig_path);
              base = baseNameOf path;
            in
              base == "stdlib.scm";
          inherit projectName;
        };
        generatedSrc = cargo2nix-ifd.lib.${system}.generateSrc {
          src = filteredSrc;
          inherit projectName rustToolchain;
        };
        rustPkgs = pkgs.rustBuilder.makePackageSet {
          packageFun = import "${generatedSrc}/Cargo.nix";
          inherit rustToolchain;
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
        webschembly-runtime = pkgs.callPackage ./webschembly-runtime { inherit webschembly-runtime-rust; BINARYEN_ARGS = pkgs.lib.strings.trim (builtins.readFile ./binaryen-args.txt); };
      in
      {
        packages = {
          inherit webschembly-compiler-cli webschembly-runtime;
          inherit rustToolchain;
        };
        defaultPackage = self.packages.${system}.webschembly-compiler-cli;
        devShell = rustPkgs.workspaceShell {
          nativeBuildInputs = [
            cargo2nix.packages.${system}.cargo2nix
          ];

          buildInputs = [
            pkgs.gnumake
            pkgs.nodejs_22
            pkgs.nixpkgs-fmt
            pkgs.binaryen
            pkgs.wasm-tools
            pkgs.cargo-insta
          ];
        };
      }
    );
}
