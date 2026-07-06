{ pkgs }:

let 
  app = pkgs.rustPlatform.buildRustPackage {
    pname = "axum-demo-hw";
    version = "0.0.2";
    src = ./app;
    cargoLock.lockFile = ./app/Cargo.lock;
    doCheck = false;
  };
in
  pkgs.dockerTools.buildLayeredImage {
    name = "axum-demo-hw";
    tag = "latest";
    contents = [
      app
      pkgs.dockerTools.caCertificates
    ];
    config = {
      Entrypoint=["${app}/bin/app"];
      Env = [
        "PORT=8080"
        "SSL_CERT_FILE=${pkgs.dockerTools.caCertificates}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  }
