{ pkgs }:

let
  app = pkgs.rustPlatform.buildRustPackage {
    pname = "axum-demo-hw";
    version = "0.0.2";
    src = ./app;
    cargoLock.lockFile = ./app/Cargo.lock;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl pkgs.onnxruntime ];
    env = {
      ORT_LIB_LOCATION = "${pkgs.onnxruntime}/lib";
      ORT_PREFER_DYNAMIC_LINK = "1";
      ORT_SKIP_DOWNLOAD = "1";
      CARGO_NET_OFFLINE = "true";
    };
    doCheck = false;
  };

  fastembedCacheSource = if builtins.pathExists ./.fastembed_cache then ./.fastembed_cache else pkgs.runCommand "empty-fastembed-cache" { } "mkdir -p $out";

  fastembedCache = pkgs.runCommand "fastembed-cache" { } ''
    mkdir -p "$out/var/cache/fastembed"
    if [ -d ${fastembedCacheSource} ]; then
      cp -R ${fastembedCacheSource}/. "$out/var/cache/fastembed/"
    fi
  '';
in
  pkgs.dockerTools.buildLayeredImage {
    name = "axum-demo-hw";
    tag = "latest";
    contents = [
      app
      fastembedCache
      pkgs.busybox
      pkgs.dockerTools.caCertificates
    ];
    config = {
      Entrypoint = [ "${app}/bin/app" ];
      Env = [
        "PORT=8080"
        "FASTEMBED_CACHE_DIR=/var/cache/fastembed"
        "SSL_CERT_FILE=${pkgs.dockerTools.caCertificates}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  }
