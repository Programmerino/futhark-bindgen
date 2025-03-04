{
  description = "futhark-bindgen";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux"];
      perSystem = {
        system,
        ...
      }: let
        pkgs = import inputs.nixpkgs {
          inherit system;
        };
      in rec {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "futhark-bindgen";
          version = "0.2.8";

          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
        };
        devShells.default = pkgs.mkShell {
          packages =
            with pkgs; [nil alejandra git futhark]
            ++ packages.default.buildInputs
            ++ packages.default.nativeBuildInputs
            ++ packages.default.propagatedBuildInputs;
        };
        formatter = pkgs.alejandra;
      };
    };
}
