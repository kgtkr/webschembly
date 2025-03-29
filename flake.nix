{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    make-shell.url = "github:nicknovitski/make-shell";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      # mainが壊れている: https://github.com/cargo2nix/cargo2nix/issues/392
      url = "github:cargo2nix/cargo2nix/8ce65922a814571dd94bd2f49910758b5b7edff2";
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
      perSystem = { self', pkgs, system, ... }:
        {
          _module.args = {
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [ inputs.cargo2nix.overlays.default inputs.napalm.overlays.default ];
            };
            cargo2nix-ifd-lib = inputs.cargo2nix-ifd.mkLib pkgs;
          };
          packages = {
            default = self'.packages.webschembly-compiler-cli;
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
