{
  description = "A devShell example";
  inputs = {
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

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
    fenix,
    advisory-db,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [fenix.overlays.default];
      };
      inherit (pkgs) lib;

      rustToolchain = pkgs.fenix.combine (with pkgs.fenix; [
        stable.cargo
        stable.clippy
        stable.rustc
        latest.rustfmt
      ]);

      craneLib = (crane.mkLib pkgs).overrideToolchain (p: rustToolchain);
      craneDev = craneLib.overrideToolchain (p:
        p.fenix.combine (with p.fenix.stable; [
          rustToolchain
          rust-analyzer
          rust-src
        ]));

      craneNightly = craneLib.overrideToolchain pkgs.fenix.minimal.toolchain;

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

      mkChecks = craneLib: let
        cargoArtifacts = mkCargoArtifacts craneLib;
      in {
        # Run clippy
        workspace-clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

        # Check formatting
        workspace-fmt = craneLib.cargoFmt {
          inherit src;
        };

        # Audit dependencies
        workspace-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };

        # Audit licenses
        workspace-deny = craneLib.cargoDeny {
          inherit src;
        };

        # Run tests with cargo-nextest
        # Consider setting `doCheck = false` on other crate derivations
        # if you do not want the tests to run twice
        workspace-nextest = craneLib.cargoNextest (commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          });
      };
    in {
      checks =
        {
          workspace-udeps = craneNightly.mkCargoDerivation (commonArgs
            // {
              cargoArtifacts = mkCargoArtifacts craneNightly;
              pnameSuffix = "-udeps";
              buildPhaseCargoCommand = "cargo udeps";
              nativeBuildInputs = [pkgs.cargo-udeps];
            });
        }
        // mkChecks craneLib
        // packages;
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

      devShells.default = craneDev.devShell {
        checks = (mkChecks craneDev) // mkPackages craneDev;

        CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.llvmPackages.clangUseLLVM}/bin/clang";
        CARGO_ENCODED_RUSTFLAGS = "-Clink-arg=-fuse-ld=${pkgs.mold}/bin/mold";

        packages = with pkgs; [
          grpcurl
          sqlx-cli
        ];
      };
    });
}
