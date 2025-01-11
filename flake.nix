{
  description = "Flake for scape";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    nix-filter.url = "github:numtide/nix-filter";
    crane = {
      url = "github:ipetkov/crane";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    nix-filter,
    crane,
    fenix,
    advisory-db,
    ...
  }:
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"] (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      craneLib = (crane.mkLib pkgs).overrideToolchain fenix.packages.${system}.stable.toolchain;

      src = nix-filter.lib.filter {
        root = ./.;
        include = [
          ./src
          ./Cargo.toml
          ./Cargo.lock
          ./deny.toml
          ./resources
        ];
      };

      pkgDef = {
        inherit src;
        nativeBuildInputs = with pkgs; [
          pkg-config
          autoPatchelfHook
          xwayland
          cargo-flamegraph
          clang
        ];
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
          pixman
          libgcc
          tracy
          just
          pipewire
          stdenv.cc.cc.lib
          libdisplay-info
          pkgs.vulkan-tools
          pkgs.vulkan-tools-lunarg
          glslang
          libGL
          vulkan-helper
          shaderc
        ];
        runtimeDependencies = with pkgs; [
          libglvnd
          wayland # needed for winit on wayland
          xorg.libX11 # needed for winit on x11
          xorg.libXcursor # needed for winit on x11
          xorg.libXrandr # needed for winit on x11
          xorg.libXi # needed for winit on x11
          pkgs.libGL
          pkgs.vulkan-headers
          pkgs.vulkan-loader
          pkgs.vulkan-extension-layer
          pkgs.vulkan-validation-layers
        ];
        LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
      };

      cargoArtifacts = craneLib.buildDepsOnly pkgDef;
      scape = craneLib.buildPackage (pkgDef
        // {
          inherit cargoArtifacts;
        });
    in {
      checks = {
        inherit scape;

        scape-clippy = craneLib.cargoClippy (pkgDef
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

        scape-doc = craneLib.cargoDoc (pkgDef
          // {
            inherit cargoArtifacts;
          });

        scape-fmt = craneLib.cargoFmt {
          inherit src;
        };

        scape-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };

        scape-deny = craneLib.cargoDeny {
          inherit src;
        };

        scape-nextest = craneLib.cargoNextest (pkgDef
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });

        # cargo nextest currently does not support doc tests
        scape-doctest = craneLib.cargoTest (pkgDef
          // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--locked --doc --workspace";
          });
      };

      packages.default = scape;

      apps.default = flake-utils.lib.mkApp {
        drv = scape;
      };

      devShells.default = pkgs.mkShell {
        inputsFrom =
          (builtins.attrValues self.checks.${system})
          ++ [
            pkgs.vscode-extensions.vadimcn.vscode-lldb.adapter
          ];
        LD_LIBRARY_PATH = pkgs.lib.strings.makeLibraryPath pkgDef.runtimeDependencies;
        LIBRARY_PATH = pkgs.lib.strings.makeLibraryPath pkgDef.runtimeDependencies;
        LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";
      };
    });
}
