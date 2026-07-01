{
  description = "A simple flake for testing";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system}.container = pkgs.callPackage ./container.nix { };
  
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          pkgs.rustc
          pkgs.cargo
          pkgs.rust-analyzer
          pkgs.clippy
          pkgs.rustfmt
          pkgs.just
          pkgs.pulumi-bin
          pkgs.google-cloud-sdk
          pkgs.gcrane
          pkgs.crane
          (pkgs.python3.withPackages (ps: with ps; [
             polars
             requests
             numpy 
          ]))
        ];
        
        shellHook = ''
          export CLOUDSDK_CONFIG="$PWD/.gcloud"
          export PULUMI_HOME="$PWD/.pulumi"
          export PATH="$PWD/.pulumi/bin:$PATH"
          export GOOGLE_APPLICATION_CREDENTIALS="$PWD/.gcloud/application_default_credentials.json"
          export DOCKER_CONFIG="$PWD/.docker-config"
          mkdir -p .gcloud .pulumi .docker-config/
          
          export PYO3_PYTHON="${pkgs.python3}/bin/python3"
        '';
      };
    };
}
