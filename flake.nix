{
  description = "A devShell example";
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
    advisory-db,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };
      inherit (pkgs) lib;

      rustToolchain = pkgs.rust-bin.stable.latest.minimal.override {
        extensions = ["clippy"];
      };
      rustDevToolchain = rustToolchain.override (prev: {
        extensions = prev.extensions ++ ["rust-docs" "rust-src" "rust-analyzer"];
      });
      rustfmt = pkgs.rust-bin.selectLatestNightlyWith (t: t.rustfmt);

      craneLib = (crane.mkLib pkgs).overrideToolchain (p: rustToolchain);
      craneDev = craneLib.overrideToolchain (p: rustDevToolchain);

      root = ./.;
      src = lib.fileset.toSource {
        inherit root;
        fileset = lib.fileset.unions [
          (craneLib.fileset.commonCargoSources ./.)
          (lib.fileset.fileFilter (file: file.hasExt "proto") ./shared/proto/proto)
          (lib.fileset.maybeMissing ./migrations)
          (lib.fileset.maybeMissing ./.sqlx)
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

        rssflow-fetch = craneLib.buildPackage (mkIndividualCrateArgs craneLib
          // {
            pname = "rssflow-fetch";
            cargoExtraArgs = "-p rssflow-fetch";

            src = fileSetForCrate ./services/fetch [
              (craneLib.fileset.commonCargoSources ./services/websub)
              (lib.fileset.fileFilter (file: file.hasExt "proto") ./services/websub)
            ];
          });
      };

      packages = mkPackages craneLib;

      dockerImages = {
        rssflow = pkgs.dockerTools.buildLayeredImage {
          name = "rssflow";
          tag = "latest";
          contents = [packages.rssflow];
          config = {
            Cmd = ["${packages.rssflow}/bin/rssflow"];
          };
        };

        rssflow-fetch = pkgs.dockerTools.buildLayeredImage {
          name = "rssflow-fetch";
          tag = "latest";
          contents = [packages.rssflow-fetch];
          config = {
            Cmd = ["${packages.rssflow-fetch}/bin/rssflow-fetch"];
          };
        };
      };

      mkChecks = craneLib: {
        # Run clippy
        workspace-clippy = craneLib.cargoClippy (commonArgs
          // {
            cargoArtifacts = mkCargoArtifacts craneLib;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

        # Check formatting
        workspace-fmt = craneLib.cargoFmt {
          inherit src;
          nativeBuildInputs = [rustfmt];
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
            cargoArtifacts = mkCargoArtifacts craneLib;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          });
      };
    in {
      checks = mkChecks craneLib // packages;
      packages =
        {
          default = packages.rssflow;
          inherit dockerImages;
        }
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
        ];
      };
    });
}
