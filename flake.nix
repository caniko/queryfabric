{
  description = "QueryFabric - portable analytical query compiler for scientific platforms";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    plinth = {
      url = "git+https://codeberg.org/caniko/plinth";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rs-harbor = {
      url = "git+https://codeberg.org/caniko/rs-harbor.git?ref=trunk&rev=b40cd4c4fdf6133962f67bd68a48bfd5d554d47f";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.crane.follows = "crane";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    advisory-db = {
      url = "github:RustSec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-parts,
      crane,
      plinth,
      treefmt-nix,
      git-hooks,
      rust-overlay,
      rs-harbor,
      advisory-db,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      perSystem =
        { system, ... }:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          lib = pkgs.lib;
          craneLib = crane.mkLib pkgs;
          sccachePackage = rs-harbor.packages.${system}.sccache;
          buildCache = rs-harbor.lib.mkBuildCachePolicy {
            inherit pkgs sccachePackage;
            buildPackageSet = pkgs.buildPackages;
            cacheRoot = null;
            namespaceScope = "canix-rust";
            namespaceGeneration = 5;
          };
          cacheRust = package: buildCache.withRustCache {inherit package;};
          cross = rs-harbor.lib.mkCross {
            inherit pkgs system;
            enableOsxcross = false;
          };
          nixfmt = pkgs.nixfmt;
          plinthProject = plinth.packages.${system}.plinth-project;
          treefmtEval = treefmt-nix.lib.evalModule pkgs (import ./nix/treefmt.nix);
          pre-commit-check = git-hooks.lib.${system}.run {
            src = ./.;
            hooks = import ./nix/pre-commit.nix {
              inherit pkgs;
              treefmtWrapper = treefmtEval.config.build.wrapper;
              rustToolchain = pkgs.rustc;
            };
          };
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

          queryfabricDemoArgs = {
            pname = "queryfabric-demo";
            version = "0.2.0";
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                (craneLib.fileset.commonCargoSources ./.)
                ./crates/queryfabric-ir/src/budget.rs
                ./crates/queryfabric-portability/src/import.rs
                ./crates/queryfabric-portability/schema
                ./crates/queryfabric-portability/fixtures
                ./crates/queryfabric-web/assets/queryfabric_syql_editor.js
                ./crates/queryfabric-runtime-k8s/tests/golden/replicated_read_only_job.json
                ./crates/queryfabric-demo/src/index.html
                ./capabilities/builtin-capability-manifest.json
                ./conformance/portable-subset.json
              ];
            };
            strictDeps = true;
            cargoExtraArgs = "-p queryfabric-demo --locked";
            meta = {
              description = "QueryFabric self-host demonstrator service";
              license = lib.licenses.asl20;
              mainProgram = "queryfabric-demo";
            };
          };
          queryfabric-demo = cacheRust (craneLib.buildPackage queryfabricDemoArgs);
          bundleSchemaArgs = {
            pname = "queryfabric-portability-schema-fixtures";
            version = "0.2.0";
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                (craneLib.fileset.commonCargoSources ./.)
                ./crates/queryfabric-portability/schema
                ./crates/queryfabric-portability/fixtures
              ];
            };
            strictDeps = true;
            cargoExtraArgs = "-p queryfabric-portability --locked";
          };
          bundleSchemaArtifacts = cacheRust (craneLib.buildDepsOnly bundleSchemaArgs);
          bundle-schema = cacheRust (craneLib.cargoTest (
            bundleSchemaArgs // { cargoArtifacts = bundleSchemaArtifacts; }
          ));
          crossLanguage =
            pkgs.runCommand "queryfabric-cross-language-vectors"
              {
                nativeBuildInputs = [
                  (pkgs.python3.withPackages (ps: [
                    ps.rfc8785
                    ps.blake3
                  ]))
                ];
              }
              ''
                set -euo pipefail
                python3 - ${./crates/queryfabric-portability/fixtures/rfc8785-vector.json} "$out" <<'PY'
                import json
                import pathlib
                import sys

                import blake3
                import rfc8785

                fixture = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
                canonical = rfc8785.dumps(fixture["input"]).decode("utf-8")
                if canonical != fixture["canonical"]:
                    raise SystemExit("Python RFC 8785 output differs from the published vector")
                digest = "blake3-256:" + blake3.blake3(canonical.encode("utf-8")).hexdigest()
                if digest != fixture["typedDigest"]:
                    raise SystemExit("Python BLAKE3 output differs from the published vector")
                pathlib.Path(sys.argv[2]).write_text("independent Python RFC 8785/BLAKE3 vector passed\n")
                PY
              '';
          msrvToolchain = pkgs.rust-bin.stable."1.94.0".default;
          msrvCraneLib = (crane.mkLib pkgs).overrideToolchain (_: msrvToolchain);
          msrvArgs = queryfabricDemoArgs // {
            cargoExtraArgs = "--workspace --exclude queryfabric-python --locked";
          };
          msrvArtifacts = cacheRust (msrvCraneLib.buildDepsOnly msrvArgs);
          # The MSRV gate is a full-workspace compile gate. Runtime tests run
          # under the stable workspace gate; keeping this check compile-only
          # avoids TLS-provider global state in unrelated test binaries.
          msrv = cacheRust (msrvCraneLib.buildPackage (
            msrvArgs
            // {
              cargoArtifacts = msrvArtifacts;
              doCheck = false;
            }
          ));
          audit = craneLib.cargoAudit {
            pname = "queryfabric-audit";
            version = "0.2.0";
            src = ./.;
            inherit advisory-db;
            cargoAuditExtraArgs = "";
          };
          deny = craneLib.cargoDeny {
            pname = "queryfabric-deny";
            version = "0.2.0";
            src = ./.;
            cargoDenyChecks = "bans licenses sources";
          };
          accessibility =
            pkgs.runCommand "queryfabric-accessibility-check"
              {
                nativeBuildInputs = [ pkgs.python3 ];
              }
              ''
                set -euo pipefail
                python3 - "${docs}" "$out" <<'PY'
                import pathlib
                import re
                import sys

                root = pathlib.Path(sys.argv[1])
                pages = sorted(page for page in root.rglob("*.html") if page.name not in {"404.html", "toc.html"})
                if not pages:
                    raise SystemExit("no generated HTML pages found")
                for page in pages:
                    html = page.read_text(encoding="utf-8")
                    if not re.search(r"<html[^>]*lang=[\"'][^\"']+[\"']", html, re.I):
                        raise SystemExit(f"{page}: missing html lang")
                    title_start = html.lower().find("<title>")
                    title_end = html.lower().find("</title>", title_start + 7)
                    if title_start < 0 or title_end < 0 or not html[title_start + 7:title_end].strip():
                        raise SystemExit(f"{page}: missing non-empty title")
                    if not re.search(r"<main", html, re.I):
                        raise SystemExit(f"{page}: missing main landmark")
                    for image in re.findall(r"<img[^>]*>", html, re.I):
                        if not re.search(r"alt=[\"'][^\"']*[\"']", image, re.I):
                            raise SystemExit(f"{page}: image without alt attribute")
                pathlib.Path(sys.argv[2]).write_text("structural accessibility gate passed\\n")
                PY
              '';
          crossPackageSet = rs-harbor.lib.mkCrossPackages (
            {
              inherit pkgs craneLib cross;
              commonArgs = queryfabricDemoArgs;
              pname = "queryfabric-demo";
              targets = [
                "native"
                "aarch64-linux"
              ];
            }
            //
              lib.optionalAttrs
                (builtins.hasAttr "toolchainArgs" (builtins.functionArgs rs-harbor.lib.mkCrossPackages))
                {
                  toolchainArgs = {
                    channel = "stable";
                    extensions = [
                      "rust-src"
                      "rustfmt"
                      "clippy"
                    ];
                  };
                }
          );

          docs = pkgs.stdenv.mkDerivation {
            pname = "queryfabric-docs";
            version = "0.1.0";
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                (lib.fileset.maybeMissing ./docs)
                ./ROADMAP.md
                ./COMPATIBILITY.md
              ];
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

          site = pkgs.stdenvNoCC.mkDerivation {
            pname = "queryfabric-site";
            version = "0.1.0";
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.maybeMissing ./website;
            };
            nativeBuildInputs = [ plinthProject ];
            phases = [
              "buildPhase"
              "installPhase"
            ];
            buildPhase = ''
              set -euo pipefail
              cp -r --no-preserve=mode "$src/website" website
              plinth-project build --config website/plinth-project.toml --out public
            '';
            installPhase = ''
              mkdir -p "$out"
              cp -r public/. "$out/"
              mkdir -p "$out/docs"
              cp -r ${docs}/. "$out/docs/"
            '';
          };
        in
        {
          formatter = nixFormatter;

          packages = {
            inherit
              docs
              site
              queryfabric-demo
              ;
            "queryfabric-demo-aarch64-linux" = crossPackageSet."queryfabric-demo-aarch64-linux";
            default = site;
            website = site;
          };

          checks = {
            # Fast gate: the demonstrator builds (and its unit tests pass)
            # on every check run.
            inherit queryfabric-demo;
            # Public schemas and cross-language JCS/digest fixtures are an
            # independent offline gate.
            inherit
              bundle-schema
              crossLanguage
              ;
            inherit
              msrv
              audit
              deny
              accessibility
              ;

            legacyAliasEval =
              let
                _ = nixpkgs.lib.nixosSystem {
                  system = pkgs.stdenv.hostPlatform.system;
                  modules = [
                    self.nixosModules.queryfabric
                    ({ ... }: {
                      services.queryfabric = {
                        enable = true;
                        database.url = "postgres://queryfabric@/queryfabric?host=/run/postgresql";
                        auth.secret = "qf-demo-auth-secret-2026-operator-000000";
                        store.backend = "memory";
                      };
                    })
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
            # Heavy gate: boot a VM with Postgres + Garage + the NixOS
            # module and drive query/export/import/GDPR end-to-end.
            selfhost = import ./nix/tests/selfhost.nix {
              inherit pkgs;
              nixosModule = self.nixosModules.queryfabric;
            };
            portability-migration = import ./nix/tests/portability-migration.nix {
              inherit pkgs;
              nixosModule = self.nixosModules.queryfabric;
            };
            module = import ./nix/tests/module.nix {
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
              plinthProject
              pkgs.reuse
              pkgs.openssl
              pkgs.pkg-config
              pkgs.python3
              pkgs.rust-analyzer
              pkgs.rustc
              pkgs.rustfmt
              pkgs.uv
            ]
            ++ pre-commit-check.enabledPackages;

            shellHook = pre-commit-check.shellHook + ''
              echo "QueryFabric dev shell"
              echo "Website: plinth-project dev --config website/plinth-project.toml"
              echo "Documentation: cd docs && mdbook serve"
            '';
          };

          apps = {
            deploy-pages = plinth.lib.${system}.mkDeployPagesApp {
              domain = "queryfabric.tartanoglu.com";
            };
          };
        };

      flake = {
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
        crossPackages."x86_64-linux"."aarch64-linux".queryfabric-demo =
          self.packages."x86_64-linux"."queryfabric-demo-aarch64-linux";
      };
    };
}
