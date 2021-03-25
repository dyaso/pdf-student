with import <nixpkgs> { };

rustPlatform.buildRustPackage rec {
  name = "pdf-student-${version}";
  version = "0.1.0";
  src = ./.;
  nativeBuildInputs = [ stdenv pkgconfig cmake 
  clang 
  wrapGAppsHook 

  #glib 
  gtk3-x11 
  # gnome3.defaultIconTheme hicolor-icon-theme 
  ];
  buildInputs = [ stdenv 
  freetype 
  expat
  pango
  atk
  gdk-pixbuf
  gtk3-x11
  x11
  libxkbcommon
  clang
  cmake
  llvmPackages.libclang

  #not sure which of the following are needed to prevent `GLib-GIO-ERROR **: Settings schema 'org.gtk.Settings.FileChooser' on opening an open file dialogue window
  wrapGAppsHook 
  #glib
  #hicolor-icon-theme  gnome3.defaultIconTheme 
  ];
    configurePhase = ''
      export LIBCLANG_PATH="${pkgs.llvmPackages.libclang}/lib";
    '';

  checkPhase = "";
  cargoSha256 = "sha256:01bnsp37px5208g4x9xrrd1qgy8sa0v0m305gmh7hds69svcvd94";
  # "sha256:0000000000000000000000000000000000000000000000000000";

strictDeps = false;

# dontWrapGApps = true;

# preFixup = ''
#     makeWrapperArgs+=("''${gappsWrapperArgs[@]}")
#   '';

  meta = with lib; {
    description = "Bare bones PDF ebook reader";
    homepage = https://github.com/dyaso/pdf-student;
    license = licenses.isc;
    maintainers = [ ];
    platforms = platforms.all;
  };
}