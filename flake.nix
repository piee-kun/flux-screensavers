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
        version = "2.26.4";
        name = "SDL2-static-${version}";
        src = builtins.fetchurl {
          url =
            "https://www.libsdl.org/release/${old.pname}-${version}.tar.gz";
          sha256 =
            "sha256:0cbji2l35j5w9v5kkb9s16n6w03xg81kj2zqygcqlxpvk1j6h3qs";
        };
        dontDisableStatic = true;
      });
    in rec {
      devShells = {
        default = pkgs.pkgsBuildHost.mkShell {
         # inputsFrom = [packages.default];

          packages = with pkgs.pkgsBuildHost; [
            rustToolchain
            pkg-config
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

          nativeBuildInputs = [
            pkgs.pkgsBuildHost.pkg-config
          ];

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
