{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "llvm-tools-preview" "rustfmt" "clippy" ];
          targets = [ "riscv64gc-unknown-none-elf" ];
        };

        tg-checker = pkgs.rustPlatform.buildRustPackage rec {
          pname = "tg-rcore-tutorial-checker";
          version = "0.4.8";
          src = pkgs.fetchCrate {
            inherit pname version;
            hash = "sha256-3N79bxI6ASCadelovnYc1bgiaVzNTIvLJftvZ77/6JQ=";
          };
          cargoHash = "sha256-AOZ+t7eLnCY/5zyKM+rdkMeC6BQHkd605O5h89rvBtQ=";
        };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.qemu
            pkgs.cargo-binutils
            tg-checker
          ];
        };
      });
}
