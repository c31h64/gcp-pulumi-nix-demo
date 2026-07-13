default:
    @just --list

auth:
    gcloud auth login
    gcloud auth application-default login
    @gcloud auth print-access-token | crane auth login -u oauth2accesstoken --password-stdin europe-west1-docker.pkg.dev
    
bootstrap: auth
    # gcloud projects create c31h64-threewhitetowers
    gcloud config set project c31h64-threewhitetowers
    # gcloud storage buckets create gs://c31h64-threewhitetowers-pulumi-state

    gcloud services enable artifactregistry.googleapis.com
    gcloud services enable run.googleapis.com
    gcloud services enable cloudbuild.googleapis.com
    gcloud services enable aiplatform.googleapis.com
    gcloud services enable compute.googleapis.com
    gcloud services enable vpcaccess.googleapis.com
    gcloud services enable networkconnectivity.googleapis.com
    gcloud services enable memorystore.googleapis.com
    gcloud services enable secretmanager.googleapis.com
        
    cd infra/
    pulumi login gs://c31h64-threewhitetowers-pulumi-state
    pulumi config set gcp:project c31h64-threewhitetowers
    cd angularcli && npm install
    cd frontend && npm install
    
    @echo "Bootstrap complete."

api-image-build:
    FASTEMBED_CACHE_PATH="$PWD/.fastembed_cache" nix build --impure .#api_image -o api-image.tar.gz

api-container-smoke-test: api-image-build
    gunzip -c ./api-image.tar.gz > api-image.tar
    docker load < result.tar
    docker rm -f axum-demo-hw-smoke >/dev/null 2>&1 || true
    docker run --rm --network host --name axum-demo-hw-smoke --entrypoint /bin/sh axum-demo-hw:latest -lc 'test -d /var/cache/fastembed && find /var/cache/fastembed -maxdepth 3 -type f | head -5 && find /nix/store -path "*/lib/libonnxruntime.so*" | head -5'
    rm -f api-image.tar

run-api-container: api-image-build
    docker rm -f axum-demo-hw-local >/dev/null 2>&1 || true
    docker run -d --network host --name axum-demo-hw-local \
        -e PORT=8080 \
        -e VALKEY_HOST=127.0.0.1 \
        -e VALKEY_PORT=6379 \
        -e GOOGLE_CLOUD_PROJECT="$(gcloud config get-value project 2>/dev/null || echo c31h64-threewhitetowers)" \
        -e GOOGLE_APPLICATION_CREDENTIALS=/gcloud/application_default_credentials.json \
        -v "$PWD/.gcloud:/gcloud:ro" \
        axum-demo-hw:latest
    @echo "Waiting for container to become ready..."
    @curl --retry 20 --retry-connrefused --retry-delay 1 --retry-all-errors -sS http://127.0.0.1:8080/health || true
    @echo "--- /health ---"
    @curl -sS -i http://127.0.0.1:8080/health || true
    @echo "--- /ready ---"
    @curl -sS -i http://127.0.0.1:8080/ready || true
    @echo "--- container logs ---"
    @docker logs axum-demo-hw-local 2>&1 | tail -50 || true

stop-api-image-container:
    docker rm -f axum-demo-hw-local >/dev/null 2>&1 || true

#############################################################################################################
build-frontend:
    cd frontend && ng build

api-image-push: api-image-build
    gunzip -c ./api-image.tar.gz > api-image.tar

    @gcloud auth print-access-token | crane auth login -u oauth2accesstoken --password-stdin europe-west1-docker.pkg.dev

    crane push ./api-image.tar europe-west1-docker.pkg.dev/c31h64-threewhitetowers/c31h64-twt-repo/axum-demo-hw:latest

    rm api-image.tar

#############################################################################################################

export GOOGLE_APPLICATION_CREDENTIALS := invocation_directory() + "/.gcloud/application_default_credentials.json"
pulumi := "cd infra && pulumi"

pulumi-deploy:
    {{pulumi}} up --yes

pulumi-preview:
    cd infra && pwd
    {{pulumi}} preview
    
pulumi-destroy:
    {{pulumi}} destroy --yes --continue-on-error

pulumi-refresh:
    {{pulumi}} refresh --yes

pulumi-stack-rmf:
    {{pulumi}} stack rm --force

pulumi-stack-init-dev:
    {{pulumi}} stack init dev

#############################################################################################################
valkey-server-docker-run:
    docker run --rm -it \
      --name valkey-search \
      --net=host \
      valkey/valkey-bundle:latest \
      valkey-server --save "" --appendonly no \
      --requirepass "$VALKEY_PASSWORD"

valkey-cli-run:
    REDISCLI_AUTH=$VALKEY_PASSWORD valkey-cli

invalidate-cdn-cache:
    #!/usr/bin/env fish
    set -l url_maps (gcloud compute url-maps list --format="value(name)" | grep url-map-)
    
    for map in $url_maps
        echo "Invalidating $map..."
        gcloud compute url-maps invalidate-cdn-cache "$map" --path="/*"
    end
