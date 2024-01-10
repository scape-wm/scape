{
  description = "Flake for scape";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    nix-filter.url = "github:numtide/nix-filter";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    nix-filter,
    crane,
    fenix,
  }:
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"] (system: let
      pkgs = import nixpkgs {inherit system;};
      craneLib = crane.lib.${system}.overrideToolchain fenix.packages.${system}.stable.toolchain;

      pkgDef = {
        src = nix-filter.lib.filter {
          root = ./.;
          include = [
            ./src
            ./Cargo.toml
            ./Cargo.lock
            ./resources
          ];
        };
        nativeBuildInputs = with pkgs; [pkg-config autoPatchelfHook];
        buildInputs = with pkgs; [
          udev
          dbus
          wayland
          xwayland
          seatd
          libxkbcommon
          libinput
          mesa
          llvmPackages.bintools
          libgcc
          glib # needed for puffin
          gtk3 # needed for puffin
          tracy # for profiling with tracy
          just
        ];
        runtimeDependencies = with pkgs; [
          libglvnd
          xorg.libX11 # needed for winit
          xorg.libXcursor # needed for winit
          xorg.libXrandr # needed for winit
          xorg.libXi # needed for winit
        ];
      };

      cargoArtifacts = craneLib.buildDepsOnly pkgDef;
      scape = craneLib.buildPackage (pkgDef
        // {
          inherit cargoArtifacts;
        });
    in {
      checks = {
        inherit scape;
      };

      packages.default = scape;

      apps.default = flake-utils.lib.mkApp {
        drv = scape;
      };

      devShells.default = pkgs.mkShell rec {
        inputsFrom = builtins.attrValues self.checks.${system};
        LD_LIBRARY_PATH = pkgs.lib.strings.makeLibraryPath (builtins.concatMap (d: d.runtimeDependencies) inputsFrom);
      };
    });

  nixConfig = {
    extra-trusted-substituters = ["https://nix-community.cachix.org"];
    extra-trusted-public-keys = ["nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="];
  };
}
