{
  description = "NATS Web UI";
  version = "0.1.2";  # the version number you want to track

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;
        devTools = with pkgs; [ docker gh ];
        nativeBuiltInputs = with pkgs; [ rustToolchain pkg-config ] ++ devTools;
        version = self.version;  # export the version

        pname = "simple-nats-ui";

        mkDockerImage = tag: pkgs.dockerTools.buildLayeredImage {
          name = pname;
          inherit tag;
          contents = [ 
            self.packages.${system}.default
            pkgs.bashInteractive
            pkgs.coreutils
          ];
          config = {
            Cmd = [ "/bin/${pname}" ];
            ExposedPorts = { "3000/tcp" = {}; };
          };
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          inherit pname version;
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          nativeBuildInputs = nativeBuiltInputs;
        };

        packages.dockerDebug = mkDockerImage "debug";
        packages.docker = mkDockerImage version;
        packages.dockerLatest = mkDockerImage "latest";

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = nativeBuiltInputs;
          shellHook = ''
            if [ -e /var/run/docker.sock ]; then
              export DOCKER_HOST="unix:///var/run/docker.sock"
            fi
          '';
        };

        version = version;  # make the version available as an output attribute
      }
    );
}
