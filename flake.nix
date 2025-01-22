{
  inputs = {
    nixpkgs = {
      url = "github:cachix/devenv-nixpkgs/rolling";
    };
    systems = {
      url = "github:nix-systems/default";
    };

    devenv = {
      url = "github:cachix/devenv";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs =
    { self
    , nixpkgs
    , devenv
    , systems
    , ...
    }@inputs:
    let
      forEachSystem = nixpkgs.lib.genAttrs (import systems);
    in
    {
      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [
              {
                env = {
                  "SERVER_CONFIG_FILE" = "server.toml";
                };

                # https://devenv.sh/reference/languages/
                languages = {
                  rust = {
                    # https://devenv.sh/reference/options/#languagesrustenable
                    enable = true;
                    channel = "nightly";
                    mold = {
                      enable = true;
                    };

                    # https://devenv.sh/reference/options/#languagesrustrustflags
                    # NOTE: This must be kept in sync with .cargo/config.toml
                    rustflags = nixpkgs.lib.strings.concatStringsSep " " [
                    ];
                  };
                };

                # https://devenv.sh/reference/options/#packages
                packages = with pkgs; [
                  just
                  static-web-server

                  # Rust
                  bacon

                  rust-analyzer

                  cargo-edit
                  cargo-outdated
                  cargo-deny
                  cargo-bloat
                  cargo-modules

                  cargo-criterion
                  cargo-flamegraph

                  cargo-fuzz
                ];

                # https://devenv.sh/reference/options/#pre-commit
                pre-commit = {
                  # https://devenv.sh/reference/options/#pre-commithooks
                  hooks = {
                    # Filesystem
                    check-symlinks = {
                      enable = true;
                    };

                    # TOML
                    check-toml = {
                      enable = true;
                    };

                    # Spelling
                    hunspell = {
                      enable = true;
                      entry = "${pkgs.hunspell}/bin/hunspell -d 'en_GB' -p .spelling/dictionary,.spelling/html,.spelling/horrors -l";
                    };

                    # Secrets
                    ripsecrets = {
                      enable = true;
                    };

                    # Nix
                    nixpkgs-fmt = {
                      enable = true;
                    };

                    # Rust
                    cargo-check = {
                      enable = true;
                    };
                    clippy = {
                      enable = true;
                      settings = {
                        allFeatures = true;
                      };
                    };
                    rustfmt = {
                      enable = true;
                    };
                  };
                };

                enterShell = ''
                  cargo --version
                  rustc --version
                '';

                enterTest = ''
                  cargo test
                '';
              }
            ];
          };
        }
      );
    };
}
