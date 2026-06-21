{
  description = "mrs — automated theorem prover (CASC) + mrs-proover (ProoVer) — reproducible dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Pin the exact stable toolchain (>= 1.95 required by Cargo.toml,
        # edition 2024). The `default` profile bundles cargo, rustfmt,
        # clippy, rust-std and rust-docs; we add rust-src + rust-analyzer
        # for editor support. This toolchain lives in the Nix store and is
        # GC-rooted by the flake, so it is immune to the ~/.rustup linker
        # shim breakage that occurs when nixpkgs `rustup` is rebuilt and the
        # old build is garbage-collected.
        rustToolchain = pkgs.rust-bin.stable."1.96.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Optional runtime libraries for the `ml`/`ml-guidance` feature builds
        # (Burn + wgpu). Not needed for the default CASC / ProoVer builds.
        mlRuntimeLibs = with pkgs; [
          vulkan-loader
          vulkan-headers
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          # stdenv supplies a C/C++ compiler, which the `cadical` crate
          # (CaDiCaL SAT solver, built from source via the `cc` crate) needs.
          nativeBuildInputs = [
            rustToolchain
            pkgs.pkg-config
          ];

          buildInputs = [
            # CaDiCaL is compiled from vendored C++ sources; no extra libs
            # required, but keep clang/llvm tooling available for any crate
            # that wants it.
          ];

          packages = with pkgs; [
            git
            cargo-nextest
          ];

          # Make the wgpu/vulkan loader discoverable for `--features
          # ml-guidance` runs. Harmless for default builds.
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath mlRuntimeLibs;

          shellHook = ''
            echo "mrs dev shell — $(rustc --version)"
            echo "  default build:  cargo build --release"
            echo "  proover build:  cargo build --release --features proover --bin mrs"
            echo "  tests:          cargo test --workspace"
          '';
        };

        # `nix fmt` formats this flake.
        formatter = pkgs.nixpkgs-fmt;
      });
}
