{
  description = "Build and development environment for Typsite";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      flake-parts,
      nixpkgs,
      rust-overlay,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      perSystem =
        {
          lib,
          system,
          self',
          ...
        }:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          packageName = cargoToml.package.name;
          packageVersion = cargoToml.package.version;
          packageDescription = cargoToml.package.description or "Static site generator for Typst";
          rustFlags = [ "--cfg" "tokio_unstable" ];
          commonNativeBuildInputs = [
            pkgs.nasm
            pkgs.perl
            pkgs.pkg-config
          ];
          darwinBuildInputs = lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconvReal ];
          nativeBuildInputs = commonNativeBuildInputs ++ darwinBuildInputs;
          libPath = lib.optionalString pkgs.stdenv.isDarwin (lib.makeLibraryPath [ pkgs.libiconvReal ]);

          mkTypsitePackage =
            targetPkgs:
            targetPkgs.rustPlatform.buildRustPackage {
              pname = packageName;
              version = packageVersion;

              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;

              inherit nativeBuildInputs;
              buildInputs = [ targetPkgs.openssl ];
              LIBRARY_PATH = libPath;
              RUSTFLAGS = lib.concatStringsSep " " rustFlags;

              doCheck = false;
              enableParallelBuilding = true;
              strictDeps = true;

              meta = {
                description = packageDescription;
                homepage = "https://github.com/Glomzzz/typsite";
                license = lib.licenses.mit;
                mainProgram = packageName;
              };
            };

          crossTargets = {
            x86_64-linux = pkgs.pkgsCross.gnu64;
            x86_64-linux-static = pkgs.pkgsCross.gnu64.pkgsStatic;

            aarch64-linux = pkgs.pkgsCross.aarch64-multiplatform;
            aarch64-linux-static = pkgs.pkgsCross.aarch64-multiplatform.pkgsStatic;

            x86_64-windows = pkgs.pkgsCross.mingwW64;
          }
          // lib.optionalAttrs pkgs.stdenv.isDarwin {
            aarch64-darwin = pkgs.pkgsCross.aarch64-darwin;
          };

          packages = {
            default = mkTypsitePackage pkgs;
            static = mkTypsitePackage pkgs.pkgsStatic;
          }
          // lib.mapAttrs (_: targetPkgs: mkTypsitePackage targetPkgs) crossTargets;
        in
        {
          inherit packages;

          apps.default = {
            type = "app";
            program = "${self'.packages.default}/bin/typsite";
            meta.description = packageDescription;
          };

          checks.default = packages.default;

          devShells.default = pkgs.mkShell {
            packages = [
              pkgs.rust-bin.stable.latest.default
              pkgs.openssl
            ]
            ++ nativeBuildInputs;
          };
        };
    };
}
