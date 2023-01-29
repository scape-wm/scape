{
  description = "Flake for setting up pkg-config dependencies";

  outputs = { self, nixpkgs }:
    let
      pkgs = import nixpkgs { system = "x86_64-linux"; };
    in
    {
      devShell."x86_64-linux" = pkgs.mkShell {
        # pkgs.mesa is needed because it provides libgdb (which could be used from other packages too)
        nativeBuildInputs = [
          pkgs.udev
          pkgs.dbus
          pkgs.wayland
          pkgs.seatd
          pkgs.pkg-config
          pkgs.libxkbcommon
          pkgs.mesa
          pkgs.libinput
          pkgs.egl-wayland
          pkgs.libGL
          pkgs.llvmPackages.bintools
        ];
      };
    };
}
