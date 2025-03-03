{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      url = "github:kgtkr/cargo2nix/396edea";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, cargo2nix, crate2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        overlays = [ cargo2nix.overlays.default ];
        pkgsArgs = {
          inherit system overlays;
        };
        pkgs = import nixpkgs pkgsArgs;
        tools = pkgs.callPackage crate2nix.lib.tools { inherit pkgs; };
        src = ./.;
        cargoToml = "Cargo.toml";
        crateDir = dirOf (src + "/${cargoToml}");
        vendor = tools.internal.vendorSupport rec {
          inherit crateDir;
          lockFiles = tools.internal.gatherLockFiles crateDir;
          hashes = tools.internal.gatherHashes (lockFiles);
        };
        cargonix = pkgs.stdenv.mkDerivation {
          name = "cargonix";

          buildInputs = [ rustToolchain cargo2nix.packages.${system}.cargo2nix ];

          inherit src;

          buildPhase = ''
            export HOME=/tmp/home
            export CARGO_HOME="$HOME/cargo"
            mkdir -p $CARGO_HOME

            cp ${vendor.cargoConfig} $CARGO_HOME/config

            CARGO_OFFLINE=true cargo2nix -o -f default.nix --locked
          '';

          installPhase = ''
            mkdir -p $out
            cp default.nix $out/
          '';

        };
        mkWebschembly =
          { release }:
          let
            rustPkgs = pkgs.rustBuilder.makePackageSet {
              rustToolchain = rustToolchain;
              packageFun = import cargonix;
              workspaceSrc = src;
              inherit release;
            };
            wasmRustPkgs = pkgs.rustBuilder.makePackageSet {
              rustToolchain = rustToolchain;
              packageFun = import cargonix;
              target = "wasm32-unknown-unknown";
              workspaceSrc = src;
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
          inherit cargonix;
          x = cargonix;
        };
        defaultPackage = webschembly.webschembly-compiler-cli;
        devShell = webschembly.workspaceShell {
          nativeBuildInputs = [
            cargo2nix.packages.${system}.cargo2nix
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.pkg-config
          ];

          buildInputs = [
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
