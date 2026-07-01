{ pkgs }:

let 
  app = pkgs.rustPlatform.buildRustPackage {
    pname = "c31h64-twt-axum-hw-cloud-run";
    version = "0.0.1";
    src = ./app;
    cargoLock.lockFile = ./app/Cargo.lock;
  };
in
  pkgs.dockerTools.buildLayeredImage {
    name = "c31h64-twt-axum-hw-cloud-run";
    tag = "0.0.1";
    contents = [ app ];
    config = {
      Entrypoint=["${app}/bin/app"];
      Env = ["PORT=8080"];
    };
  }
