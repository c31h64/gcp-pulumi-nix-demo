{ pkgs }:

let 
  app = pkgs.rustPlatform.buildRustPackage {
    pname = "axum-demo-hw";
    version = "0.0.1";
    src = ./app;
    cargoLock.lockFile = ./app/Cargo.lock;
  };
in
  pkgs.dockerTools.buildLayeredImage {
    name = "axum-demo-hw";
    tag = "latest";
    contents = [ app ];
    config = {
      Entrypoint=["${app}/bin/app"];
      Env = ["PORT=8080"];
    };
  }
