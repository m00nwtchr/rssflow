{
  description = "A devShell example";
  nixConfig = {
    extra-substituters = [
      "https://m00nwtchr.cachix.org"
    ];
    extra-trusted-public-keys = [
      "m00nwtchr.cachix.org-1:obUPSTOPq11tzLSzMCHBq/A2PTeIv9qIZW1IxCeb8Yw="
    ];
  };
  inputs = {
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };
  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };
      inherit (pkgs) lib;

      craneLib = (crane.mkLib pkgs).overrideToolchain (p: p.rust-bin.stable.latest.minimal);

      root = ./.;
      src = lib.fileset.toSource {
        inherit root;
        fileset = lib.fileset.unions [
          (craneLib.fileset.commonCargoSources ./.)
          (lib.fileset.fileFilter (file: file.hasExt "proto") ./shared/proto/proto)
          (lib.fileset.maybeMissing ./migrations)
          (lib.fileset.maybeMissing ./.sqlx)
          (lib.fileset.maybeMissing ./rustfmt.toml)
        ];
      };

      rustHostPlatform = pkgs.hostPlatform.rust.rustcTarget;

      # Common arguments can be set here to avoid repeating them later
      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs = with pkgs; [] ++ lib.optionals stdenv.isDarwin [];

        nativeBuildInputs = with pkgs; [
          protobuf
        ];
      };

      mkCargoArtifacts = craneLib: craneLib.buildDepsOnly commonArgs;

      mkIndividualCrateArgs = craneLib:
        commonArgs
        // {
          cargoArtifacts = mkCargoArtifacts craneLib;
          inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
          # NB: we disable tests since we'll run them all via cargo-nextest
          doCheck = false;
        };

      fileSetForCrate = crate: fs:
        lib.fileset.toSource {
          inherit root;
          fileset = lib.fileset.unions ([
              ./Cargo.toml
              ./Cargo.lock

              (craneLib.fileset.commonCargoSources ./shared)
              (lib.fileset.fileFilter (file: file.hasExt "proto") ./shared/proto/proto)
              (craneLib.fileset.commonCargoSources ./src)

              (craneLib.fileset.commonCargoSources crate)
              (lib.fileset.maybeMissing (crate + /.sqlx))
              (lib.fileset.maybeMissing (crate + /migrations))
            ]
            ++ fs);
        };

      mkPackage = craneLib: name:
        craneLib.buildPackage (mkIndividualCrateArgs craneLib
          // {
            pname = "rssflow-${name}";
            cargoExtraArgs = "-p rssflow-${name}";

            src = fileSetForCrate ./services/${name} [];
          });

      mkPackages = craneLib: {
        rssflow = craneLib.buildPackage (mkIndividualCrateArgs craneLib
          // {
            pname = "rssflow";
            cargoExtraArgs = "-p rssflow";

            src = fileSetForCrate ./src [
              (craneLib.fileset.commonCargoSources ./services/dummy)
              (lib.fileset.maybeMissing ./migrations)
              (lib.fileset.maybeMissing ./.sqlx)
            ];
          });

        rssflow-websub = mkPackage craneLib "websub";
        rssflow-fetch = mkPackage craneLib "fetch";
        rssflow-filter = mkPackage craneLib "filter";
        rssflow-replace = mkPackage craneLib "replace";
        rssflow-retrieve = mkPackage craneLib "retrieve";
        rssflow-sanitize = mkPackage craneLib "sanitize";
      };

      packages = mkPackages craneLib;

      mkImage = name: pkg:
        pkgs.dockerTools.buildLayeredImage {
          name = name;
          tag = "latest";
          contents = [pkg];
          config = {
            Cmd = ["${pkg}/bin/${name}"];
          };
        };

      dockerImages = lib.mapAttrs mkImage packages;
    in {
      packages =
        {
          default = packages.rssflow;
          dockerImages = pkgs.linkFarm "docker-images" (pkgs.lib.mapAttrsToList
            (name: image: {
              name = name;
              path = image;
            })
            dockerImages);
        }
        // (lib.mapAttrs' (name: value: {
            name = "${name}-docker";
            value = value;
          })
          dockerImages)
        // packages;
      apps.default = flake-utils.lib.mkApp {
        drv = packages.rssflow;
      };
    });
}
