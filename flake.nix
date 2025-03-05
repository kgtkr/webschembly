{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      url = "github:kgtkr/cargo2nix/396edea";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    # for tools.nix
    crate2nix = {
      url = "github:nix-community/crate2nix";
      flake = false;
    };
    # for lib/filterCargoSources.nix
    crane = {
      url = "github:ipetkov/crane";
      flake = false;
    };
  };

  outputs = { nixpkgs, flake-utils, cargo2nix, crate2nix, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        overlays = [ cargo2nix.overlays.default ];
        pkgsArgs = {
          inherit system overlays;
        };
        pkgs = import nixpkgs pkgsArgs;
        tools = pkgs.callPackage "${crate2nix}/tools.nix" { inherit pkgs; };
        src = ./.;
        filterCargoSources = pkgs.callPackage "${crane}/lib/filterCargoSources.nix" {};
        filterWebschembly = orig_path: type:
          let
            path = (toString orig_path);
            base = baseNameOf path;
          in
            base == "stdlib.scm";
        filter = path: type: filterCargoSources path type || filterWebschembly path type;
        filteredSrc = pkgs.lib.cleanSourceWith {
          name = "webschembly-src";
          inherit src filter;
        };
        vendor = tools.internal.vendorSupport rec {
          crateDir = filteredSrc;
          lockFiles = [ "${crateDir}/Cargo.lock" ];
        };
        generatedSrc = pkgs.stdenv.mkDerivation {
          name = "webschembly-generated-src";
          buildInputs = [ rustToolchain cargo2nix.packages.${system}.cargo2nix ];
          src = filteredSrc;
          buildPhase = ''
            export HOME=/tmp/home
            export CARGO_HOME="$HOME/cargo"
            mkdir -p $CARGO_HOME

            cp ${vendor.cargoConfig} $CARGO_HOME/config
            CARGO_OFFLINE=true cargo2nix --locked
          '';

          installPhase = ''
            mkdir -p $out

            cp -r $src/* $out/
            cp Cargo.nix $out/
          '';

        };
        mkWebschembly =
          { release }:
          let
            rustPkgs = pkgs.rustBuilder.makePackageSet {
              rustToolchain = rustToolchain;
              packageFun = import "${generatedSrc}/Cargo.nix";
              inherit release;
            };
            wasmRustPkgs = pkgs.rustBuilder.makePackageSet {
              rustToolchain = rustToolchain;
              packageFun = import "${generatedSrc}/Cargo.nix";
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
