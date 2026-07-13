{
  description = "A hands-on learning experiment exploring modern backend, infrastructure, and API patterns with Rust, cloud-native tools, and generative AI (Google Gemini, Vertex AI).";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system}.api_image = pkgs.callPackage ./api_image.nix { };

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
          pkgs.nodejs_26
          pkgs.valkey
          pkgs.openssl    # Required by ort-sys / libssl
          pkgs.pkg-config # Required to find the openssl library
          pkgs.gitleaks
        ];
        
        shellHook = ''
          export CLOUDSDK_CONFIG="$PWD/.gcloud"
          export PULUMI_HOME="$PWD/.pulumi"
          export PATH="$PWD/.pulumi/bin:$PATH"
          #export GOOGLE_APPLICATION_CREDENTIALS="$PWD/.gcloud/application_default_credentials.json"
          export DOCKER_CONFIG="$PWD/.docker-config"
          export FASTEMBED_CACHE_DIR="$PWD/.fastembed_cache"
          
          mkdir -p .gcloud .pulumi .docker-config/ .fastembed_cache/

          export GOOGLE_CLOUD_PROJECT=$(gcloud config get-value project)
          export GOOGLE_CLOUD_LOCATION="global"

          export PULUMI_CONFIG_PASSPHRASE=$(cat ".pulumi-pass-plaintext.txt" | tr -d '\n')
          export VALKEY_HOST="localhost"
          export VALKEY_PORT="6379"
          export VALKEY_PASSWORD=$(cat ".valkey-pass-plaintext.txt" | tr -d '\n')
          
          export PYO3_PYTHON="${pkgs.python3}/bin/python3"
          export PATH="$PWD/angularcli/node_modules/.bin:$PATH"
        '';
      };
    };
}
