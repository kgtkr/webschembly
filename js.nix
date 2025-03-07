{ lib, ... }: {
  perSystem = { self', pkgs, ... }:
    let
      nodejs = pkgs.nodejs_22;
      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions ([
          ./package.json
          ./package-lock.json
        ] ++ lib.map (lib.path.append ./.) (builtins.fromJSON (builtins.readFile ./package.json)).workspaces);
      };
      generatedSrc = pkgs.napalm.buildPackage src {
        inherit nodejs;
        name = "webschembly-node_modules";
      };
      webschembly-playground = pkgs.stdenv.mkDerivation {
        name = "webschembly-playground";
        buildInputs = [ pkgs.gnumake nodejs ];
        src = "${generatedSrc}/_napalm-install";
        buildPhase = ''
          make -C webschembly-playground WEBSCHEMBLY_RUNTIME=${self'.packages.webschembly-runtime}/lib/webschembly_runtime.wasm
        '';
        installPhase = ''
          mkdir -p $out
          cp -r webschembly-playground/dist/* $out
        '';
      };
    in
    {
      packages = {
        inherit webschembly-playground;
        webschembly-playground-for-pages = webschembly-playground.overrideAttrs (oldAttrs: {
          BASE_URL = "/webschembly/";
        });
      };
      make-shells.default = {
        packages = [
          nodejs
        ];
      };
    };
}
