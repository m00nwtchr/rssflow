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
  packages = [pkgs.git pkgs.cargo-nextest];

  # https://devenv.sh/languages/
  languages.rust.enable = true;

  # https://devenv.sh/processes/
  # processes.cargo-watch.exec = "cargo-watch";

  # https://devenv.sh/services/
  # services.postgres.enable = true;

  # https://devenv.sh/scripts/
  scripts.hello.exec = ''
    echo hello from $GREET
  '';

  enterShell = ''
    hello
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
    git --version | grep --color=auto "${pkgs.git.version}"
    cargo nextest run --verbose --workspace --all-features
  '';

  # https://devenv.sh/git-hooks/
  git-hooks.hooks = {
    clippy.enable = true;
  };

  # See full reference at https://devenv.sh/reference/options/
}
