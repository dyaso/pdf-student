with import <nixpkgs> { };

rustPlatform.buildRustPackage rec {
  name = "pdf-student-${version}";
  version = "0.1.0";
  src = ./.;

BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${llvmPackages.libclang.lib}/lib/clang/${lib.getVersion clang}/include";

  LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";

  nativeBuildInputs = [ stdenv pkgconfig cmake 
  clang 
  wrapGAppsHook 
libclang
  python3
  #glib 
  gtk3-x11 
  llvmPackages.libclang
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
  libclang

  #not sure which of the following are needed to prevent `GLib-GIO-ERROR **: Settings schema 'org.gtk.Settings.FileChooser' on opening an open file dialogue window
  wrapGAppsHook 
  #glib
  #hicolor-icon-theme  gnome3.defaultIconTheme 
  ];
    configurePhase = ''
      export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";
    '';

  checkPhase = "";
  cargoSha256 = "sha256:0k8y98anzqyg87v6agsbz4z83004plh1d1vxyqg602ivgdablrhw";
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