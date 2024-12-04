{
  description = "NATS Web UI";

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

        devTools = with pkgs; [
          docker
          gh
        ];

        nativeBuiltInputs = with pkgs; [
          rustToolchain
          pkg-config
        ] ++ devTools;

        version = "0.1.2";
        pname = "simple-nats-ui";

        # Function to create a docker image with a specific tag
        mkDockerImage = tag: pkgs.dockerTools.buildLayeredImage {
          name = pname;
          inherit tag;
          contents = [ 
            self.packages.${system}.default
            pkgs.bashInteractive  # Add shell for debugging
            pkgs.coreutils       # Add basic utilities
          ];

          config = {
            Cmd = [ "/bin/${pname}" ];
            ExposedPorts = {
              "3000/tcp" = {};
            };
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

        # Debug version with shell
        packages.dockerDebug = mkDockerImage "debug";
        # Production versions without shell
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
      }
    );
}