{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [
                rust-overlay.overlays.default
                self.overlays.default
              ];
            };
          }
        );
    in
    {
      nix.nixPath = [
        "nixpkgs=${nixpkgs}"
      ];

      overlays.default = final: prev: {
        rustToolchain = prev.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell rec {
            buildInputs = with pkgs; [
              rustToolchain
            ];

            packages = with pkgs; [
              # Spelling
              hunspell
              hunspellDicts.en_GB-large

              # Project
              static-web-server
              just

              # Git
              ripsecrets

              # Nix
              nixfmt

              # Rust
              bacon
              cargo-bloat
              cargo-criterion
              cargo-deny
              cargo-edit
              cargo-flamegraph
              cargo-modules
              cargo-outdated
            ];

            env = {
              # Bevy (https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md#nix)
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

              # Spelling
              DICTIONARY = "en_GB";
              DICPATH = "${pkgs.hunspell}/bin/hunspell";

              # Rust
              # RUSTFLAGS = "-C target-cpu=native";  # NOTE: This ruins reproducibility
              ## Required by rust-analyzer
              RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };
          };
        }
      ); # ..devShells

    };
}
