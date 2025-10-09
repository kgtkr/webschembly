{ ... }: {
  perSystem = { self', pkgs, ... }:
    let
      tex = pkgs.texlive.combine {
        inherit (pkgs.texlive) scheme-small
          luatexbase
          luatexja
          lualatex-math
          #tikz
          pgf
          #font
          haranoaji
          ;
      };
    in
    {
      packages = {
        webschembly-docs = pkgs.stdenv.mkDerivation {
          name = "webschembly-docs";
          src = ./docs;
          buildInputs = [ tex pkgs.pandoc pkgs.gnumake ];
          buildPhase = ''
            make index.pdf
          '';
          installPhase = ''
            mkdir -p $out
            cp index.pdf $out/
          '';
        };
      };
      make-shells.default = {
        packages = [
          tex
          pkgs.pandoc
        ];
      };
    };
}
