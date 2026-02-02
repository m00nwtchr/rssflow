{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  name = "rssflow";

  root = ./services;

  entries = builtins.readDir root;

  serviceNames = lib.filter (
    name: entries.${name} == "directory" && builtins.pathExists (root + "/${name}/Cargo.toml")
  ) (builtins.attrNames entries);
in
{
  cachix.enable = true;
  cachix.pull = [ "m00nwtchr" ];

  # https://devenv.sh/packages/
  packages =
    with pkgs;
    [
    ]
    ++ lib.optionals (!config.container.isBuilding) [
      git
      cargo-nextest
      sqlx-cli
      watchexec
    ];

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    mold.enable = true;
  };

  env.PGDATABASE = name;
  env.DATABASE_URL = "postgresql:///${config.env.PGDATABASE}?host=${config.env.PGHOST}";
  processes = {
    rssflow.exec = "watchexec -r -e rs -- cargo run -p rssflow";
  }
  // (lib.genAttrs serviceNames (name: {
    exec = "watchexec -r -e rs -- cargo run -p rssflow-${name}";
  }));
  services.postgres = {
    enable = true;
    initialDatabases = [
      {
        name = config.env.PGDATABASE;
      }
    ];
  };
  services.redis = {
    enable = true;
    package = pkgs.valkey;
  };

  treefmt = {
    enable = true;
    config.programs = {
      nixfmt.enable = true;
      rustfmt.enable = true;
    };
  };

  git-hooks.hooks = {
    treefmt.enable = true;
    clippy.enable = true;
  };

  tasks = {
    # "${name}:tests" = {
    #   after = ["devenv:enterTest"];
    #   exec = "cargo nextest run";
    # };
  };

  # outputs = let
  #   cargoNix = inputs.crate2nix.tools.${pkgs.stdenv.system}.appliedCargoNix {
  #     inherit name;
  #     src = ./.;
  #   };
  # in {
  #   rssflow = cargoNix.rootCrate.build;
  # };
  # See full reference at https://devenv.sh/reference/options/
}
