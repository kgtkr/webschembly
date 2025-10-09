{ lib, ... }: {
  perSystem = { self', pkgs, ... }:
    {
      packages = { };
      make-shells.default = {
        packages = [
          pkgs.texlive.combined.scheme-full
          pkgs.pandoc
        ];
      };
    };
}
