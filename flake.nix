{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    make-shell.url = "github:nicknovitski/make-shell";
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

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      imports = [
        inputs.make-shell.flakeModules.default
        ./rust.nix
        ./js.nix
      ];
      perSystem = { config, pkgs, system, ... }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.cargo2nix.overlays.default inputs.napalm.overlays.default ];
          };
          packages = {
            default = config.packages.webschembly-compiler-cli;
          };
          make-shells.default = {
            packages = [
              pkgs.gnumake
              pkgs.nixpkgs-fmt
              pkgs.binaryen
              pkgs.wasm-tools
            ];
          };
        };
    };
}
