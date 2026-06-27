{pkgs, ...}: {
  projectRootFile = "flake.nix";

  programs.rustfmt = {
    enable = true;
    edition = "2024";
    package = pkgs.rustfmt;
  };

  programs.alejandra.enable = true;

  programs.taplo.enable = true;
  programs.taplo.package = pkgs.taplo;

  programs.prettier = {
    enable = true;
    package = pkgs.prettier;
    includes = [
      "*.md"
      "*.markdown"
      "*.yaml"
      "*.yml"
    ];
  };
}
