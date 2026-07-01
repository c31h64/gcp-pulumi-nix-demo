default:
    @just --list

auth:
    gcloud auth login
    gcloud auth application-default login

bootstrap: auth
    # gcloud projects create c31h64-threewhitetowers
    gcloud config set project c31h64-threewhitetowers
    # gcloud storage buckets create gs://c31h64-threewhitetowers-pulumi-state
    pulumi login gs://c31h64-threewhitetowers-pulumi-state
    @echo "Bootstrap complete."

deploy:
    pulumi up
