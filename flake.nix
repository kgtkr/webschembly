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

  outputs = { self, nixpkgs, flake-utils, cargo2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        overlays = [ cargo2nix.overlays.default ];
        pkgsArgs = {
          inherit system overlays;
        };
        pkgs = import nixpkgs pkgsArgs;
        wasmPkgs = import nixpkgs (pkgsArgs // {
          crossSystem = {
            system = "wasm32-wasi";
            useLLVM = true;
          };
        });
        rustPkgs = pkgs.rustBuilder.makePackageSet {
          rustToolchain = rustToolchain;
          packageFun = import ./Cargo.nix;
        };
        wasmRustPkgs = wasmPkgs.rustBuilder.makePackageSet {
          rustToolchain = rustToolchain;
          packageFun = import ./Cargo.nix;
          target = "wasm32-unknown-unknown";
        };
      in
      {
        packages = {
          cli = (rustPkgs.workspace.webschembly-compiler-cli { }).bin;
          runtime = (wasmRustPkgs.workspace.webschembly-runtime { }).out;
        };
        defaultPackage = self.packages.${system}.webschembly-compiler-cli;
        devShell = rustPkgs.workspaceShell {
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
          ];
        };
      }
    );
}
