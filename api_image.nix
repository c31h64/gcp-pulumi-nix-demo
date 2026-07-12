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

  fastembedCachePath = builtins.getEnv "FASTEMBED_CACHE_PATH";
  expectedFastembedModelPath = fastembedCachePath + "/models--Xenova--all-MiniLM-L6-v2/snapshots";

  fastembedCacheSource =
    if fastembedCachePath != "" then
      if builtins.pathExists fastembedCachePath then
        if builtins.pathExists expectedFastembedModelPath then
          builtins.path {
            path = fastembedCachePath;
            name = "fastembed-cache-source";
          }
        else
          builtins.abort "Fastembed cache is missing the model snapshot at ${expectedFastembedModelPath}"
      else
        builtins.abort "Fastembed cache directory is missing at ${fastembedCachePath}"
    else
      builtins.abort "FASTEMBED_CACHE_PATH must point to a directory containing the fastembed model cache";

  fastembedCache = pkgs.runCommand "fastembed-cache" { } ''
    mkdir -p "$out/var/cache/fastembed"
    cp -R ${fastembedCacheSource}/. "$out/var/cache/fastembed/"
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
