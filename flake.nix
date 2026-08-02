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

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Parse Cargo.toml dynamically
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        rustVersion =
          if builtins.isString (cargoToml.package.rust-version or null) then
            cargoToml.package.rust-version
          else
            cargoToml.workspace.package.rust-version or "latest";

        rustToolchain =
          (
            if rustVersion == "latest" then
              pkgs.rust-bin.stable.latest.default
            else
              pkgs.rust-bin.stable.${rustVersion}.default
          ).override
            {
              extensions = [
                "rust-src"
                "rust-analyzer"
              ];
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
            # Ensure AVX2 is enabled by default for local development,
            # appending to RUSTFLAGS so we don't clobber any user-level
            # global configurations (like custom linkers in ~/.cargo/config.toml)
            export RUSTFLAGS="''${RUSTFLAGS:-} -C target-cpu=native"

            echo "mrs dev shell — $(rustc --version)"
            echo "  default build:  cargo build --release"
            echo "  proover build:  cargo build --release --features proover --bin mrs"
            echo "  tests:          cargo test --workspace"
          '';
        };

        # `nix fmt` formats this flake.
        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
