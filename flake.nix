{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    crate2nix.url = "github:nix-community/crate2nix";
  };

  outputs = { self, nixpkgs, flake-utils, cargo2nix, crate2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ cargo2nix.overlays.default ];
        pkgsArgs = {
          inherit system overlays;
        };
        pkgs = import nixpkgs pkgsArgs;
        wasmPkgs = import nixpkgs (pkgsArgs // {
          crossSystem = pkgs.lib.systems.examples.wasi32 // { rustc.config = "wasm32-unknown-unknown"; };
        });
        mkWorkspace =
          { pkgs }:
          let
            rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
            buildRustCrateForPkgs = crate: pkgs.buildRustCrate.override {
              rustc = rustToolchain;
              cargo = rustToolchain;
            };
            generatedCargoNix = crate2nix.tools.${system}.generatedCargoNix {
              name = "Cargo.nix";
              src = ./.;
            };
            cargoNix = import generatedCargoNix {
              inherit pkgs buildRustCrateForPkgs;
            };
          in
          {
            inherit (cargoNix) workspaceMembers;
            inherit rustToolchain;
          };
        workspace = mkWorkspace { inherit pkgs; };
        wasmWorkspace = mkWorkspace { pkgs = wasmPkgs; };
        webschembly-compiler-cli = workspace.workspaceMembers.webschembly-compiler-cli.build;
        webschembly-runtime-rust = wasmWorkspace.workspaceMembers.webschembly-runtime-rust.build;
        webschembly-runtime = pkgs.callPackage ./webschembly-runtime { inherit webschembly-runtime-rust; BINARYEN_ARGS = pkgs.lib.strings.trim (builtins.readFile ./binaryen-args.txt); };
      in
      {
        packages = {
          inherit webschembly-compiler-cli webschembly-runtime-rust webschembly-runtime;
        };
        defaultPackage = self.packages.${system}.webschembly-compiler-cli;
        devShell = pkgs.mkShell {
          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.pkg-config
          ];

          buildInputs = [
            workspace.rustToolchain
            pkgs.gnumake
            pkgs.nodejs_22
            pkgs.nixpkgs-fmt
            pkgs.binaryen
            pkgs.wasm-tools
            pkgs.cargo-insta
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.glibc
          ];
        };
      }
    );
}
