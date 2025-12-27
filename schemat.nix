{ rustPlatform, fetchFromGitHub }:
rustPlatform.buildRustPackage rec {
  pname = "raviqqe";
  version = "0.4.7";

  src = fetchFromGitHub {
    owner = "raviqqe";
    repo = "schemat";
    rev = "v${version}";
    hash = "sha256-veGrwwERnMy+60paF/saEbVxTDyqNVT1hsfggGCzZt0=";
  };

  cargoHash = "sha256-R43i06XW3DpP+6fPUo/CZhKOVXMyoTPuygJ01BpW1/I=";

}

