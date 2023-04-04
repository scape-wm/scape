{
  description = "Flake for setting up pkg-config dependencies";

  outputs = { self, nixpkgs }:
    let
      pkgs = import nixpkgs { system = "x86_64-linux"; };
    in
    {
      devShell."x86_64-linux" = with pkgs; pkgs.mkShell {
        buildInputs = [
          udev
          dbus
          wayland
          seatd
          pkg-config
          libxkbcommon
          mesa
          libinput
          egl-wayland
          libGL
          llvmPackages.bintools
        ];
        LD_LIBRARY_PATH = with pkgs; lib.strings.makeLibraryPath [
          libglvnd
          /* needed for winit */
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
        ];
      };
    };
}
