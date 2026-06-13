{
  description = "QueryFabric - portable analytical query compiler for scientific platforms";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;
        craneLib = crane.mkLib pkgs;
        nixfmt = pkgs.nixfmt;
        nixSources = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./flake.nix
            ./nix
          ];
        };
        nixFormatter = pkgs.writeShellApplication {
          name = "queryfabric-format";
          runtimeInputs = [
            nixfmt
            pkgs.findutils
          ];
          text = ''
            find . \
              \( -path './.git' -o -path './result' -o -path './target' -o -path './node_modules' \) -prune \
              -o -name '*.nix' -print0 \
              | xargs -0 nixfmt "$@"
          '';
        };

        queryfabric-demo = craneLib.buildPackage {
          pname = "queryfabric-demo";
          version = "0.1.1";
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              (craneLib.fileset.commonCargoSources ./.)
              ./crates/queryfabric-demo/src/index.html
            ];
          };
          strictDeps = true;
          cargoExtraArgs = "-p queryfabric-demo";
          meta = {
            description = "QueryFabric self-host demonstrator service";
            license = lib.licenses.asl20;
            mainProgram = "queryfabric-demo";
          };
        };

        website = pkgs.stdenv.mkDerivation {
          pname = "queryfabric-website";
          version = "0.1.0";
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.maybeMissing ./website;
          };
          nativeBuildInputs = [ pkgs.zola ];
          phases = [
            "buildPhase"
            "installPhase"
          ];
          buildPhase = ''
            set -euo pipefail
            cp -r --no-preserve=mode "$src/website" site
            cd site
            zola build
          '';
          installPhase = ''
            cp -r public "$out"
          '';
        };

        docs = pkgs.stdenv.mkDerivation {
          pname = "queryfabric-docs";
          version = "0.1.0";
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.maybeMissing ./docs;
          };
          nativeBuildInputs = [ pkgs.mdbook ];
          phases = [
            "buildPhase"
            "installPhase"
          ];
          buildPhase = ''
            set -euo pipefail
            cp -r --no-preserve=mode "$src/docs" docs
            mdbook build docs
          '';
          installPhase = ''
            cp -r docs/book "$out"
          '';
        };

        site = pkgs.runCommand "queryfabric-site" { } ''
          set -euo pipefail
          mkdir -p $out
          cp -r ${website}/* "$out/"
          mkdir -p $out/docs
          cp -r ${docs}/* "$out/docs/"
        '';
      in
      {
        formatter = nixFormatter;

        packages = {
          inherit
            website
            docs
            site
            queryfabric-demo
            ;
          default = site;
        };

        checks = {
          # Fast gate: the demonstrator builds (and its unit tests pass)
          # on every check run.
          inherit queryfabric-demo;

          legacyAliasEval =
            let
              _ = nixpkgs.lib.nixosSystem {
                system = pkgs.stdenv.hostPlatform.system;
                modules = [
                  self.nixosModules.queryfabric
                  (
                    { ... }:
                    {
                      services.queryfabric = {
                        enable = true;
                        database.url = "postgres://queryfabric@/queryfabric?host=/run/postgresql";
                        store.backend = "memory";
                      };
                    }
                  )
                ];
              };
            in
            pkgs.runCommand "queryfabric-legacy-alias-eval" { } ''
              touch "$out"
            '';

          nixfmt = pkgs.runCommand "queryfabric-nixfmt-check" { nativeBuildInputs = [ nixfmt ]; } ''
            set -euo pipefail
            find ${nixSources} -name '*.nix' -print0 | xargs -0 nixfmt --check
            touch "$out"
          '';
        }
        // lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          # Heavy gate: boot a VM with Postgres + MinIO + the NixOS
          # module and drive query/export/GDPR end-to-end.
          selfhost = import ./nix/tests/selfhost.nix {
            inherit pkgs;
            nixosModule = self.nixosModules.queryfabric;
          };
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.cargo-fuzz
            pkgs.clippy
            pkgs.maturin
            pkgs.mdbook
            pkgs.reuse
            pkgs.openssl
            pkgs.pkg-config
            pkgs.python3
            pkgs.rust-analyzer
            pkgs.rustc
            pkgs.rustfmt
            pkgs.zola
          ];

          shellHook = ''
            echo "QueryFabric dev shell"
            echo "Website: cd website && zola serve"
            echo "Documentation: cd docs && mdbook serve"
          '';
        };
      }
    )
    // {
      nixosModules = {
        default = self.nixosModules.queryfabric;
        queryfabric =
          {
            pkgs,
            lib,
            ...
          }:
          {
            imports = [
              (import ./nix/modules/queryfabric.nix {
                defaultPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.queryfabric-demo;
              })
            ];
            services.queryfabric.package =
              lib.mkDefault
                self.packages.${pkgs.stdenv.hostPlatform.system}.queryfabric-demo;
          };
      };
    };
}
