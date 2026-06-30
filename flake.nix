{
  description = "Timba development environment with native Wayland support";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # Libraries needed at runtime by winit / egui
      runtimeLibs = with pkgs; [
        wayland
        libxkbcommon
        libGL
        vulkan-loader

        # Fallbacks for XWayland compatibility if needed
        xorg.libX11
        xorg.libXcursor
        xorg.libXi
        xorg.libXrandr
      ];
    in {
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          pkg-config
          cargo
          rustc
        ];

        buildInputs = runtimeLibs;

        shellHook = ''
          # Link the required graphical drivers and libraries
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}:$LD_LIBRARY_PATH"

          # Force winit to prefer Wayland over X11
          export WINIT_UNIX_BACKEND=wayland
        '';
      };
    };
}