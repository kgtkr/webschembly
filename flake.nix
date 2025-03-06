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
    napalm.url = "github:nix-community/napalm";
  };

  outputs = { self, nixpkgs, flake-utils, cargo2nix, cargo2nix-ifd, napalm, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ cargo2nix.overlays.default napalm.overlays.default ];
        };
        inherit (pkgs) lib stdenv;
        nodejs = pkgs.nodejs_22;
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        projectName = "webschembly";
        staticTarget = pkgs.pkgsStatic.stdenv.hostPlatform.config;
        mkSrc = srcs: stdenv.mkDerivation {
          name = "src";
          phases = [ "installPhase" ];
          installPhase = ''
            mkdir -p $out
            ${lib.concatStringsSep "\n" (lib.map (name: "cp -r ${lib.path.append ./. name} $out/${name}") srcs)}
          '';
        };
        filteredSrc = cargo2nix-ifd.lib.${system}.filterSrc {
          src = mkSrc (["Cargo.toml" "Cargo.lock"] ++ (fromTOML (builtins.readFile ./Cargo.toml)).workspace.members);
          orFilter = path: _type:
            let
              files = ["stdlib.scm"];
            in
              lib.any (file: baseNameOf path == file) files;
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
        webschembly-runtime = pkgs.callPackage ./webschembly-runtime { inherit webschembly-runtime-rust; BINARYEN_ARGS = lib.strings.trim (builtins.readFile ./binaryen-args.txt); };
        webschembly-node_modules = pkgs.napalm.buildPackage (mkSrc (["package.json" "package-lock.json"] ++ (builtins.fromJSON (builtins.readFile ./package.json)).workspaces)) {
          inherit nodejs;
          name = "webschembly-node_modules";
        };
        webschembly-playground = stdenv.mkDerivation {
          name = "webschembly-playground";
          buildInputs = [ pkgs.gnumake nodejs ];
          src = "${webschembly-node_modules}/_napalm-install";
          buildPhase = ''
            make -C webschembly-playground WEBSCHEMBLY_RUNTIME=${webschembly-runtime}/lib/webschembly_runtime.wasm
          '';
          installPhase = ''
            mkdir -p $out
            cp -r webschembly-playground/dist/* $out
          '';
        };
      in
      {
        packages = {
          inherit webschembly-compiler-cli webschembly-runtime webschembly-playground;
          webschembly-playground-for-pages = webschembly-playground.overrideAttrs (oldAttrs: {
            BASE_URL = "/webschembly/";
          });
        };
        defaultPackage = self.packages.${system}.webschembly-compiler-cli;
        devShell = rustPkgs.workspaceShell {
          nativeBuildInputs = [
            cargo2nix.packages.${system}.cargo2nix
          ];

          buildInputs = [
            pkgs.gnumake
            nodejs
            pkgs.nixpkgs-fmt
            pkgs.binaryen
            pkgs.wasm-tools
            pkgs.cargo-insta
          ];
        };
      }
    );
}
