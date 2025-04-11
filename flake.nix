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

      rustToolchainFor = p:
        p.rust-bin.stable.latest.minimal.override {
          extensions = ["clippy"];
        };
      rustDevToolchainFor = p:
        (rustToolchainFor p).override {
          extensions = ["rust-docs" "rust-src" "rust-analyzer"];
        };
      rustfmt = pkgs.rust-bin.selectLatestNightlyWith (t: t.rustfmt);

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchainFor;
      craneDev = craneLib.overrideToolchain rustDevToolchainFor;

      src = craneLib.cleanCargoSource self;

      # Common arguments can be set here to avoid repeating them later
      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs = with pkgs; [] ++ lib.optionals stdenv.isDarwin [];

        nativeBuildInputs = with pkgs; [];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      individualCrateArgs =
        commonArgs
        // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
          # NB: we disable tests since we'll run them all via cargo-nextest
          doCheck = false;
        };

      fileSetForCrate = crate:
        lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock

            (craneLib.fileset.commonCargoSources crate)
            (lib.fileset.maybeMissing ./migrations)
          ];
        };

      rssflow = craneLib.buildPackage (individualCrateArgs
        // {
          pname = "rssflow";
          cargoExtraArgs = "-p rssflow";
          src = fileSetForCrate ./src;
        });
    in {
      checks = {
        inherit rssflow;

        # Run clippy
        workspace-clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
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
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          });
      };

      packages.default = rssflow;
      apps.default = flake-utils.lib.mkApp {
        drv = rssflow;
      };

      devShells.default = craneDev.devShell {
        checks = self.checks.${system};

        packages = [];
      };
    });
}
