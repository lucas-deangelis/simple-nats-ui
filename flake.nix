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

        # Development tools
        devTools = with pkgs; [
          docker
          gh
        ];

        nativeBuiltInputs = with pkgs; [
          rustToolchain
          pkg-config
        ] ++ devTools;  # Add development tools to nativeBuiltInputs

      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "simple-nats-ui";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          nativeBuildInputs = nativeBuiltInputs;
        };

        packages.docker = pkgs.dockerTools.buildLayeredImage {
          name = "simple-nats-ui";
          tag = "latest";
          contents = [ self.packages.${system}.default ];

          config = {
            Cmd = [ "/bin/simple-nats-ui" ];
            ExposedPorts = {
              "3000/tcp" = {};
            };
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = nativeBuiltInputs;
          
          # Add any shell-specific environment variables
          shellHook = ''
            # Ensure docker socket is accessible if needed
            if [ -e /var/run/docker.sock ]; then
              export DOCKER_HOST="unix:///var/run/docker.sock"
            fi
          '';
        };
      }
    );
}