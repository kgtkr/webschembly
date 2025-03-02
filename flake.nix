{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs = { nixpkgs, flake-utils, cargo2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        overlays = [ cargo2nix.overlays.default ];
        pkgsArgs = {
          inherit system overlays;
        };
        pkgs = import nixpkgs pkgsArgs;
        mkWebschembly =
          { release }:
          let
            rustPkgs = pkgs.rustBuilder.makePackageSet {
              rustToolchain = rustToolchain;
              packageFun = import ./Cargo.nix;
              inherit release;
            };
            wasmRustPkgs = pkgs.rustBuilder.makePackageSet {
              rustToolchain = rustToolchain;
              packageFun = import ./Cargo.nix;
              target = "wasm32-unknown-unknown";
              inherit release;
            };
            webschembly-compiler-cli = (rustPkgs.workspace.webschembly-compiler-cli { }).bin;
            webschembly-runtime-rust = (wasmRustPkgs.workspace.webschembly-runtime-rust { }).out;
            webschembly-runtime = pkgs.callPackage ./webschembly-runtime { inherit webschembly-runtime-rust; BINARYEN_ARGS = pkgs.lib.strings.trim (builtins.readFile ./binaryen-args.txt); };
          in
          {
            inherit webschembly-compiler-cli webschembly-runtime;
            inherit (rustPkgs) workspaceShell;
          };
        webschembly = mkWebschembly { release = true; };
        webschembly-debug = mkWebschembly { release = false; };
      in
      {
        packages = {
          inherit (webschembly) webschembly-compiler-cli webschembly-runtime;
          webschembly-compiler-cli-debug = webschembly-debug.webschembly-compiler-cli;
          webschembly-runtime-debug = webschembly-debug.webschembly-runtime;
        };
        defaultPackage = webschembly.webschembly-compiler-cli;
        devShell = webschembly.workspaceShell {
          nativeBuildInputs = [
            cargo2nix.packages.${system}.cargo2nix
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.glibc
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
