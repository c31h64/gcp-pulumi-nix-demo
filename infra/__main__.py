"""A Google Cloud Python Pulumi program"""

import pulumi
import pulumi_gcp as gcp
import pulumi_command as command
import pulumi_synced_folder as psf

EUROPE_WEST1 = "europe-west1"
LOCATION = EUROPE_WEST1
 
frontend_bucket = gcp.storage.Bucket("frontend-bucket",
    website=gcp.storage.BucketWebsiteArgs(main_page_suffix="index.html"),
    uniform_bucket_level_access=True,
    location=LOCATION
)

frontend_bucket_sync = psf.GoogleCloudFolder("frontend-bucket-sync",
    psf.GoogleCloudFolderArgs(
        path="../frontend/dist/twt-ui-app/browser",
        bucket_name=frontend_bucket.name)
)

website_backend = gcp.compute.BackendBucket('website-backend',
    bucket_name=frontend_bucket.name,
    enable_cdn=False)

repo = gcp.artifactregistry.Repository(
    "c31h64-threewhitetowers-repo",
    location=LOCATION,
    repository_id="c31h64-twt-repo",
    format="DOCKER"
)

push_image = command.local.Command("push-docker-image",
    create="just push",
    triggers=[pulumi.asset.FileArchive("../app")],
    opts=pulumi.ResourceOptions(depends_on=[repo]))

image_name = repo.name.apply(
    lambda name: f"{LOCATION}-docker.pkg.dev/{gcp.config.project}/c31h64-twt-repo/axum-demo-hw:latest"
)

# IAM has eventual consistency!
startup_probe = gcp.cloudrun.ServiceTemplateSpecContainerStartupProbeArgs(
    http_get=gcp.cloudrun.ServiceTemplateSpecContainerStartupProbeHttpGetArgs(
        path="/ready"    
    ),
    initial_delay_seconds=15,
    timeout_seconds=30,
    failure_threshold=10,
    period_seconds=30
)

liveness_probe = gcp.cloudrun.ServiceTemplateSpecContainerLivenessProbeArgs(
    http_get=gcp.cloudrun.ServiceTemplateSpecContainerLivenessProbeHttpGetArgs(
        path="/health",
    ),
    initial_delay_seconds=5,
    timeout_seconds=5,
    failure_threshold=3,
    period_seconds=15
)

gemini_sa = gcp.serviceaccount.Account("gemini-sa",
    account_id="gemini-access-sa",
    display_name="Service account for Gemini API access")

sa_iam = gcp.projects.IAMMember("gemini-sa-iam",
    project=gcp.config.project,
    role="roles/aiplatform.user",
    member=gemini_sa.email.apply(lambda email: f"serviceAccount:{email}"))

gcp_env = gcp.cloudrun.ServiceTemplateSpecContainerEnvArgs(
    name="GOOGLE_CLOUD_PROJECT",
    value=gcp.config.project
)

#gcl_env = gcp.cloudrun.ServiceTemplateSpecContainerEnvArgs(
#    name="GOOGLE_CLOUD_LOCATION",
#    value="global"
#)

service = gcp.cloudrun.Service(
    "c31h64-twt-axum-demo-hw-service",
    location=LOCATION,
    template=gcp.cloudrun.ServiceTemplateArgs(
        spec=gcp.cloudrun.ServiceTemplateSpecArgs(
            service_account_name=gemini_sa.email,
            containers=[
                gcp.cloudrun.ServiceTemplateSpecContainerArgs(
                    image=image_name,
                    envs=[gcp_env],
                    startup_probe=startup_probe,
                    liveness_probe=liveness_probe
                )
            ]
        )
    ),
    opts=pulumi.ResourceOptions(depends_on=[frontend_bucket_sync, push_image, gemini_sa])
)

api_neg = gcp.compute.RegionNetworkEndpointGroup("api-neg",
    region=LOCATION,
    cloud_run=gcp.compute.RegionNetworkEndpointGroupCloudRunArgs(service=service.name)
)

api_backend = gcp.compute.BackendService("api-backend",
     load_balancing_scheme="EXTERNAL",
     backends=[
         gcp.compute.BackendServiceBackendArgs(group=api_neg.id)]
)

url_map = gcp.compute.URLMap("url-map",
     default_service=website_backend.id,
     path_matchers=[{
        "name": "routing-matcher",
        "default_service": website_backend.id,
        "path_rules": [{
            "paths": ["/quote", "/adjudicate"],
            "service": api_backend.id, # API path goes to Cloud Run
        }],
    }],
    host_rules=[{
        "hosts": ["*"],
        "path_matcher": "routing-matcher",
    }]
)

global_ip = gcp.compute.GlobalAddress("twt-global-ip", name="c31h64-twt-global-ip")

target_proxy = gcp.compute.TargetHttpProxy("http-proxy", url_map=url_map.id)

forwarding_rule = gcp.compute.GlobalForwardingRule("http-rule",
                                                   target=target_proxy.id,
                                                   ip_address=global_ip.address,
                                                   port_range="80")

iam_backend = gcp.cloudrun.IamMember(
     "c31h64-twt-public-access",
     service = service.name,
     location = service.location,
     role = "roles/run.invoker",
     member = "allUsers"
)

iam_frontend = gcp.storage.BucketIAMMember("public-bucket-access",
    bucket=frontend_bucket.name,
    role="roles/storage.objectViewer",
    member="allUsers"
)

pulumi.export("url", service.statuses.apply(lambda s: s[0].url))
pulumi.export("load_balancer_ip", global_ip.address)
