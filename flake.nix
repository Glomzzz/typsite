# flake.nix
{
  description = "Build and development environment for Typsite";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        # Import nixpkgs with the rust-overlay
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
        };

        projectName = "typsite";
        projectVersion = "0.1.7";
        commonRustFlags = ["--cfg" "tokio_unstable"];
        rustToolchain = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rust-src" "clippy" "rustfmt"];
        };

        customRustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        projectSrc = pkgs.lib.cleanSourceWith {
          src = self;
          filter = path: type: let
            baseName = baseNameOf (toString path);
          in
            ! (pkgs.lib.elem baseName [
              ".git"
              "target"
              "result"
              ".direnv"
              ".envrc"
              "tests"
            ])
            && ! (pkgs.lib.hasSuffix ".nix" baseName)
            && ! (pkgs.lib.hasSuffix ".lock" baseName && baseName != "Cargo.lock");
        };
      in {
        packages.default = customRustPlatform.buildRustPackage {
          pname = projectName;
          version = projectVersion;

          src = projectSrc;

          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [
            pkgs.pkg-config
          ];
          RUSTFLAGS = pkgs.lib.concatStringsSep " " commonRustFlags;

          meta = {
            description = "Typsite";
            homepage = "https://typsite.skillw.com";
            license = pkgs.lib.licenses.mit;
          };
        };

        packages.typsite = self.packages.${system}.default;

        # Development shell
        devShells.default = pkgs.mkShell {
          name = "${projectName}-dev";

          packages = [
            rustToolchain # Provides rustc, cargo, clippy, rustfmt
            pkgs.rust-analyzer

            pkgs.pkg-config
            pkgs.openssl
            pkgs.git
            pkgs.bashInteractive

            self.packages.${system}.default
          ];

          shellHook = ''
            echo "Entering ${projectName} (v${projectVersion}) development environment..."
            echo "Rust toolchain: $(rustc --version)"
            echo "Typst version: $(typst --version || echo 'Typst not found in PATH immediately, but available')"

            # Configure rust-analyzer
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"

            echo ""
            echo "To build the project with Nix: nix build .#default"
            echo "The binary will be in ./result/bin/${projectName}"
            echo ""
            echo "For iterative development, use standard cargo commands:"
            echo "  cargo build"
            echo "  cargo run"
            echo "  cargo test"
            echo "  cargo clippy"
            echo ""
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
