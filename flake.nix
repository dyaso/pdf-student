#
{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    #nixpkgs.url = "github:nixos/nixpkgs?ref=release-21.11";    
    rust-overlay = { url = "github:oxalica/rust-overlay"; };

  };

  nixConfig.bash-prompt = "\\[\\e[0;36m\\]druid native\\[\\e[1;37m\\] ❄ \\[\\e[0;36m\\]\\w $ \\[\\e[m\\]";
   # https://nixos.wiki/wiki/Flakes#Setting_the_bash_prompt_like_nix-shell

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in with pkgs; rec {
        devShell = mkShell rec {
          buildInputs = [
            libxkbcommon
      #      libGL

    pkgconfig udev #alsaLib lutris
cairo pango atk gdk-pixbuf 
    gtk3-x11
     xlibsWrapper# replaces `x11`
clang   
cargo 

            # WINIT_UNIX_BACKEND=wayland
     #       wayland
    xlibsWrapper# replaces `x11`

            # WINIT_UNIX_BACKEND=x11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            xorg.libX11
          ];
    #      LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
            #from https://discourse.nixos.org/t/how-to-correctly-populate-a-clang-and-llvm-development-environment-using-nix-shell/3864
    # why do we need to set the library path manually?
    shellHook = ''
      export LIBCLANG_PATH="${libclang.lib}/lib";
    '';

        };
      });
}