{
  description = "Flux Screensavers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/23.05-pre";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
  }:
    nixpkgs.lib.recursiveUpdate {
      devShells.aarch64-darwin.default = let
        pkgs = import nixpkgs {
          system = "aarch64-darwin";
          overlays = [(import rust-overlay)];
        };

        rustToolchain = pkgs.pkgsBuildHost.rust-bin.stable.latest.default;
      in
        pkgs.mkShell {
          packages = with pkgs; [rustToolchain alejandra];
        };
    } (flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"] (system: let
      pkgs = import nixpkgs {
        inherit system;
        crossSystem.config = "x86_64-w64-mingw32";
        overlays = [(import rust-overlay)];
      };

      rustToolchain = pkgs.pkgsBuildHost.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-src"
          "cargo"
          "rustc"
          "rls"
          "rust-analyzer"
          "rustfmt"
        ];
        targets = ["x86_64-pc-windows-gnu"];
      };

      craneLib = (crane.mkLib pkgs).overrideScope' (final: prev: {
        rustc = rustToolchain;
        cargo = rustToolchain;
        rustfmt = rustToolchain;
      });

      SDL2_static = pkgs.SDL2.overrideAttrs (old: rec {
        version = "2.26.5";
        name = "SDL2-static-${version}";
        src = builtins.fetchurl {
          url =
            "https://www.libsdl.org/release/${old.pname}-${version}.tar.gz";
          sha256 =
            "sha256:1xxbbvn0jmw5fgn26fyybc7xd3xsnjk67lxi8lychr5yl4yym3xd";
        };
        dontDisableStatic = true;

        # When statically linking for Windows, rust-sdl2 expects the library to be called 'SDL2-static'.
        # https://github.com/Rust-SDL2/rust-sdl2/blob/ffa4eb0b15439463561014f2d3c9d9171059d492/sdl2-sys/build.rs#L237-L238
        postInstall = ''
          mv $out/lib/libSDL2.a $out/lib/libSDL2-static.a
          mv $out/lib/libSDL2.dll.a $out/lib/libSDL2-static.dll.a
        '';
      });
    in rec {
      devShells = {
        default = pkgs.pkgsBuildHost.mkShell {
         # inputsFrom = [packages.default];

          packages = with pkgs.pkgsBuildHost; [
            rustToolchain
            fontconfig
            cmake
            alejandra
          ];

          RUSTFLAGS = "-L ${SDL2_static}/lib";
        };
      };

      packages = {
        default = craneLib.buildPackage {
          src = ./windows;
          release = true;
          doCheck = false;

          buildInputs = [
            pkgs.windows.pthreads
            pkgs.windows.mingw_w64_pthreads
            SDL2_static
          ];

          CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${pkgs.stdenv.cc.targetPrefix}cc";
          RUSTFLAGS = "-L ${SDL2_static}/lib";

          # Change the extension to .scr (Windows screensaver)
          postInstall = ''
            if [[ $out != *"deps"* ]]; then
              cp $out/bin/Flux.exe "$out/bin/Flux.scr"
            fi
          '';
        };
      };
    }));
}
