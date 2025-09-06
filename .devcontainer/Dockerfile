FROM mcr.microsoft.com/vscode/devcontainers/base:ubuntu-22.04

ARG USERNAME=vscode

RUN apt update -y && \
    apt install curl xz-utils -y && \
    mkdir /nix && \
    chown ${USERNAME} /nix

USER ${USERNAME}

RUN curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install > /tmp/install-nix && \
    sh /tmp/install-nix --no-daemon && \
    echo ". $HOME/.nix-profile/etc/profile.d/nix.sh" >> $HOME/.bashrc && \
    mkdir -p $HOME/.config/nix && \
    echo "experimental-features = nix-command flakes" >> $HOME/.config/nix/nix.conf

RUN nix profile install nixpkgs#direnv nixpkgs#nix-direnv && \
    echo ". $HOME/.nix-profile/share/nix-direnv/direnvrc" >> $HOME/.bashrc
