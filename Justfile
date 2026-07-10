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
    
    cd infra/
    pulumi login gs://c31h64-threewhitetowers-pulumi-state
    pulumi config set gcp:project c31h64-threewhitetowers
    cd angularcli && npm install
    cd frontend && npm install
    
    @echo "Bootstrap complete."

invalidate-cdn-cache:
    #!/usr/bin/env fish
    set -l url_maps (gcloud compute url-maps list --format="value(name)" | grep url-map-)
    
    for map in $url_maps
        echo "Invalidating $map..."
        gcloud compute url-maps invalidate-cdn-cache "$map" --path="/*"
    end

build:
    nix build .#container -o result.tar.gz

container-smoke-test: build
    gunzip -c ./result.tar.gz > result.tar
    docker load < result.tar
    docker rm -f axum-demo-hw-smoke >/dev/null 2>&1 || true
    docker run --rm --network host --name axum-demo-hw-smoke --entrypoint /bin/sh axum-demo-hw:latest -lc 'test -d /var/cache/fastembed && find /var/cache/fastembed -maxdepth 3 -type f | head -5 && find /nix/store -path "*/lib/libonnxruntime.so*" | head -5'
    rm -f result.tar

run-container: build
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

stop-container:
    docker rm -f axum-demo-hw-local >/dev/null 2>&1 || true

build-frontend:
    cd frontend && ng build

push: build
    gunzip -c ./result.tar.gz > result.tar

    @gcloud auth print-access-token | crane auth login -u oauth2accesstoken --password-stdin europe-west1-docker.pkg.dev

    crane push ./result.tar europe-west1-docker.pkg.dev/c31h64-threewhitetowers/c31h64-twt-repo/axum-demo-hw:latest

    rm result.tar
    
deploy:
    cd infra && GOOGLE_APPLICATION_CREDENTIALS="../.gcloud/application_default_credentials.json" pulumi up --yes

preview:
    cd infra && GOOGLE_APPLICATION_CREDENTIALS="../.gcloud/application_default_credentials.json" pulumi preview
    
destroy:
    cd infra && GOOGLE_APPLICATION_CREDENTIALS="../.gcloud/application_default_credentials.json" pulumi destroy

valkey-run:
    # docker run --rm -it --name valkey-search --net=host valkey/valkey-bundle:latest valkey-server --loadmodule /usr/lib/valkey/libsearch.so --save "" --appendonly no
    docker run --rm -it --name valkey-search --net=host valkey/valkey-bundle:latest valkey-server --save "" --appendonly no
