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

build:
    nix build .#container -o result.tar.gz

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
