{
  pkgs,
  lib,
  config,
  inputs,
  ...
}: {
  # https://devenv.sh/basics/
  env.GREET = "devenv";

  cachix.enable = true;
  cachix.pull = ["m00nwtchr"];

  # https://devenv.sh/packages/
  packages = with pkgs; [git cargo-nextest cargo-audit];

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "stable";
    version = "latest";
    mold.enable = true;
  };

  # https://devenv.sh/services/
  # services.postgres.enable = true;

  enterShell = ''
    git --version
  '';

  # https://devenv.sh/tasks/
  # tasks = {
  #   "myproj:setup".exec = "mytool build";
  #   "devenv:enterShell".after = [ "myproj:setup" ];
  # };

  # https://devenv.sh/tests/
  enterTest = ''
    echo "Running tests"
    cargo nextest run --verbose --workspace --all-features
  '';

  # https://devenv.sh/git-hooks/
  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };

  # See full reference at https://devenv.sh/reference/options/
}
