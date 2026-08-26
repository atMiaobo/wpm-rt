{
  description = "Live system-wide WPM overlay for Hyprland and Quickshell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };

        wpmRt = pkgs.rustPlatform.buildRustPackage {
          pname = "wpm-rt";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };

        qml = pkgs.writeText "wpm-rt-shell.qml" (
          builtins.replaceStrings
            [ "@wpmRtBin@" ]
            [ "${wpmRt}/bin/wpm-rt" ]
            (builtins.readFile ./quickshell/shell.qml)
        );

        wpmRtShell = pkgs.writeShellApplication {
          name = "wpm-rt-shell";
          runtimeInputs = [ pkgs.quickshell ];
          text = ''
            exec quickshell -p ${qml}
          '';
        };
      in
      {
        packages = {
          default = wpmRtShell;
          daemon = wpmRt;
        };

        apps = {
          default = {
            type = "app";
            program = "${wpmRtShell}/bin/wpm-rt-shell";
          };
          daemon = {
            type = "app";
            program = "${wpmRt}/bin/wpm-rt";
          };
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.quickshell
          ];
        };
      });
}
