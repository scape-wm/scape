{
  description = "Flake for setting up pkg-config dependencies";

  outputs = { self, nixpkgs }:
    let
      pkgs = import nixpkgs { system = "x86_64-linux"; };
    in
    {
      devShell."x86_64-linux" = pkgs.mkShell {
        nativeBuildInputs = [ pkgs.udev pkgs.dbus pkgs.pkg-config ];
      };
    };
}
