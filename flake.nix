{
  description = "QueryFabric - portable analytical query compiler for scientific platforms";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        lib = pkgs.lib;
        craneLib = crane.mkLib pkgs;

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
          nativeBuildInputs = [pkgs.zola];
          phases = ["buildPhase" "installPhase"];
          buildPhase = ''
            cp -r --no-preserve=mode $src/website site
            cd site
            zola build
          '';
          installPhase = ''
            cp -r public $out
          '';
        };

        docs = pkgs.stdenv.mkDerivation {
          pname = "queryfabric-docs";
          version = "0.1.0";
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.maybeMissing ./docs;
          };
          nativeBuildInputs = [pkgs.mdbook];
          phases = ["buildPhase" "installPhase"];
          buildPhase = ''
            cp -r --no-preserve=mode $src/docs docs
            mdbook build docs
          '';
          installPhase = ''
            cp -r docs/book $out
          '';
        };

        site = pkgs.runCommand "queryfabric-site" {} ''
          mkdir -p $out
          cp -r ${website}/* $out/
          mkdir -p $out/docs
          cp -r ${docs}/* $out/docs/
        '';
      in {
        packages = {
          inherit website docs site queryfabric-demo;
          default = site;
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            cargo-fuzz
            clippy
            maturin
            mdbook
            openssl
            pkg-config
            python3
            rust-analyzer
            rustc
            rustfmt
            zola
          ];

          shellHook = ''
            echo "QueryFabric dev shell"
            echo "Website: cd website && zola serve"
            echo "Documentation: cd docs && mdbook serve"
          '';
        };
      }
    );
}
