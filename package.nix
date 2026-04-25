{
  pkgs ? import <nixpkgs> {},
  lib,
  ...
}:
pkgs.rustPlatform.buildRustPackage rec {
  pname = "weave";
  version = let
    crate_name = pname + "-cli";
  in
    (builtins.fromTOML (lib.readFile "${src}/crates/${crate_name}/Cargo.toml")).package.version;

  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = { };
  };

  # disable tests
  checkType = "debug";
  doCheck = false;

  nativeBuildInputs = with pkgs; [
    installShellFiles
    pkg-config

    llvmPackages.clang
    clang
  ];
  buildInputs = with pkgs; [
    openssl
    pkg-config

    (rust-bin.stable.latest.default)
  ];

  # postInstall = ''
  #   installShellCompletion --cmd ${pname} \
  #     --bash ./autocompletion/${pname}.bash \
  #     --fish ./autocompletion/${pname}.fish \
  #     --zsh  ./autocompletion/_${pname}
  # '';
}
