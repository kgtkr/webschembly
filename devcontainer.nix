{ lib, ... }: {
  perSystem = { self', pkgs, system, config, ... }:
    {
      packages = {
        stream-devcontainer = pkgs.dockerTools.streamLayeredImage {
          name = "webschembly-devcontainer";
          tag = "latest";
          created = "now";
          maxLayers = 20;
          includeNixDB = true;
          # nix-prefetch-docker --image-name mcr.microsoft.com/vscode/devcontainers/base --image-tag ubuntu-22.04
          fromImage = pkgs.dockerTools.pullImage {
            imageName = "mcr.microsoft.com/vscode/devcontainers/base";
            imageDigest = "sha256:ea0615c10a5f04649532bf84aca5e1d987357bc76f29d490ac3890f45f7fbf37";
            hash = "sha256-kopW8mc6zf4mBmVI2J7z50KCpsltwNIoPoltmk0J53I=";
            finalImageName = "mcr.microsoft.com/vscode/devcontainers/base";
            finalImageTag = "ubuntu-22.04";
          };
          contents = [
            # ubuntuでは /bin は /usr/bin のsymlinkなので置き換えられないようにする
            (pkgs.buildEnv {
              name = "root-env";
              extraPrefix = "/usr";
              pathsToLink = [ "/bin" ];
              paths = [
                pkgs.nix
                pkgs.direnv
              ] ++ config.make-shells.default.packages;
            })
            (pkgs.writeTextDir "etc/nix/nix.conf" ''
              sandbox = false
              trusted-users = root vscode
              experimental-features = nix-command flakes
            '')
            # base imageの.bashrcに追記したいが、それを行う手段がないため.bash_aliasesを作成する
            # base imageの.bashrcで.bash_aliasesが読み込まれるかつ、もともと存在しないファイルなので都合が良い
            (pkgs.writeTextDir "home/vscode/.bash_aliases" ''
              eval "$(direnv hook bash)"
              . "${pkgs.nix-direnv}/share/nix-direnv/direnvrc"
            '')
          ];
          fakeRootCommands = ''
            chown -R 1000:1000 ./nix ./home/vscode/
          '';
          config = {
            Env = lib.mapAttrsToList (name: value: "${name}=${value}") config.make-shells.default.env;
          };
      };
    };
  };
}
