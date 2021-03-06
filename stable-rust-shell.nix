 #this file is for NixOS users, invoke it using:
#     nix-shell stable-rust-shell.nix -I rustoverlay=~/languages/rust/nixpkgs-mozilla
# where last bit is where `git clone https://github.com/mozilla/nixpkgs-mozilla.git` was downloaded

# i put the following at the end of `/etc/nixos/configuration.nix` :
#     programs.bash.shellAliases = {
#         ns = "nix-shell stable*.nix -I rustoverlay=~/languages/rust/nixpkgs-mozilla";
#     };

# this script found at https://eipi.xyz/blog/rust-overlay-nix-shell/

with import <nixpkgs> {};

stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = [
    libxkbcommon
wrapGAppsHook
    # for druid
    cairo
    pango
    atk
    gdk-pixbuf
    gtk3-x11

    #needed for `shello` examplec
    #glib

    pkgconfig
    x11
  ];
  buildInputs = [ 
    gtk3-x11
    latest.rustChannels.stable.rust
    # latest.rustChannels.nightly.rust
    xorg.libXi
    xorg.libXrandr
    xorg.libXcursor
    wrapGAppsHook
    clang
    llvmPackages.libclang
  ];

  #from https://discourse.nixos.org/t/how-to-correctly-populate-a-clang-and-llvm-development-environment-using-nix-shell/3864
    # why do we need to set the library path manually?
    shellHook = ''
      export LIBCLANG_PATH="${pkgs.llvmPackages.libclang}/lib";
    '';

  RUST_BACKTRACE = 1;

# dontWrapGApps = true;
# preFixup = ''
#     makeWrapperArgs+=("''${gappsWrapperArgs[@]}")
#   '';

  }

