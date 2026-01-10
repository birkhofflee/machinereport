{
  description = "Fast system fetch tool in Rust for Linux";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forEachSystem = nixpkgs.lib.genAttrs systems;
    pkgsForEach = nixpkgs.legacyPackages;
  in {
    packages = forEachSystem (system: let
      pkgs = pkgsForEach.${system};
    in {
      default = self.packages.${system}.machinereport;
      machinereport = pkgs.callPackage ./nix/package.nix {};
      machinereport-mold = pkgs.callPackage ./nix/package.nix {useMold = true;};
    });

    devShells = forEachSystem (system: {
      default = pkgsForEach.${system}.callPackage ./nix/shell.nix {};
    });
  };
}
