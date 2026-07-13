import pulumi
import pulumi_gcp as gcp
import pulumi_random as random
import pulumi_command as command
import pulumi_synced_folder as psf

EUROPE_WEST1 = "europe-west1"
LOCATION = EUROPE_WEST1

app_vpc = gcp.compute.Network(
    "twt-app-vpc",
    auto_create_subnetworks=False,
    description="Private VPC for the app and Memorystore",
)

app_subnet = gcp.compute.Subnetwork(
    "twt-app-subnet",
    ip_cidr_range="10.1.0.0/24",
    region=LOCATION,
    network=app_vpc.id,
    private_ip_google_access=True,
    opts=pulumi.ResourceOptions(depends_on=[app_vpc]),
)

valkey_service_connection_policy = gcp.networkconnectivity.ServiceConnectionPolicy(
    "twt-valkey-service-connection-policy",
    name="c31h64-twt-valkey-policy",
    location=LOCATION,
    network=app_vpc.id,
    service_class="gcp-memorystore",
    psc_config={
        "subnetworks": [app_subnet.id],
    },
    opts=pulumi.ResourceOptions(
        depends_on=[app_vpc, app_subnet],
        delete_before_replace=True,
    ),
)

valkey_instance = gcp.memorystore.Instance(
    "twt-valkey-instance-2",
    instance_id="c31h64-twt-valkey-2",
    shard_count=1,
    desired_auto_created_endpoints=[
        {
            "network": app_vpc.id,
            "project_id": gcp.config.project,
        }
    ],
    location=LOCATION,
    node_type="SHARED_CORE_NANO",
    transit_encryption_mode="SERVER_AUTHENTICATION",
    authorization_mode="IAM_AUTH",
    engine_version="VALKEY_9_0",
    deletion_protection_enabled=False,
    mode="CLUSTER",
    opts=pulumi.ResourceOptions(
        depends_on=[valkey_service_connection_policy],
        # import_=f"{LOCATION}/c31h64-twt-valkey",
    ),
)

valkey_password = random.RandomPassword(
    "valkey-generated-password",
    length=32,
    special=False,
).result

valkey_secret = gcp.secretmanager.Secret(
    "valkey-password-secret",
    secret_id="valkey-password",
    replication=gcp.secretmanager.SecretReplicationArgs(
        auto=gcp.secretmanager.SecretReplicationAutoArgs()
    ),
)

valkey_secret_version = gcp.secretmanager.SecretVersion(
    "valkey-password-secret-version",
    secret=valkey_secret.id,
    secret_data=valkey_password,
)

frontend_bucket = gcp.storage.Bucket(
    "frontend-bucket",
    website=gcp.storage.BucketWebsiteArgs(main_page_suffix="index.html"),
    uniform_bucket_level_access=True,
    location=LOCATION,
)

frontend_bucket_sync = psf.GoogleCloudFolder(
    "frontend-bucket-sync",
    psf.GoogleCloudFolderArgs(
        path="../frontend/dist/twt-ui-app/browser", bucket_name=frontend_bucket.name
    ),
)

website_backend = gcp.compute.BackendBucket(
    "website-backend", bucket_name=frontend_bucket.name, enable_cdn=False
)

repo = gcp.artifactregistry.Repository(
    "c31h64-threewhitetowers-repo",
    location=LOCATION,
    repository_id="c31h64-twt-repo",
    format="DOCKER",
)

push_image = command.local.Command(
    "push-docker-image",
    create="just api-image-push",
    triggers=[pulumi.asset.FileArchive("../app")],
    opts=pulumi.ResourceOptions(depends_on=[repo]),
)

image_name = repo.name.apply(
    lambda name: (
        f"{LOCATION}-docker.pkg.dev/{gcp.config.project}/c31h64-twt-repo/axum-demo-hw:latest"
    )
)

# IAM has eventual consistency!
startup_probe = gcp.cloudrunv2.ServiceTemplateContainerStartupProbeArgs(
    http_get=gcp.cloudrunv2.ServiceTemplateContainerStartupProbeHttpGetArgs(
        path="/ready"
    ),
    initial_delay_seconds=15,
    timeout_seconds=30,
    failure_threshold=10,
    period_seconds=30,
)

liveness_probe = gcp.cloudrunv2.ServiceTemplateContainerLivenessProbeArgs(
    http_get=gcp.cloudrunv2.ServiceTemplateContainerLivenessProbeHttpGetArgs(
        path="/health",
    ),
    initial_delay_seconds=5,
    timeout_seconds=5,
    failure_threshold=3,
    period_seconds=15,
)

cloud_run_sa = gcp.serviceaccount.Account(
    "cloud-run-sa",
    account_id="cloudrun-access-sa",
    display_name="Service account for the main CloudRun API access",
)

cloud_run_aiplatform_sa_iam = gcp.projects.IAMMember(
    "cloud-run-sa-aiplatform-iam",
    project=gcp.config.project,
    role="roles/aiplatform.user",
    member=cloud_run_sa.email.apply(lambda email: f"serviceAccount:{email}"),
)

secret_accessor_iam = gcp.secretmanager.SecretIamMember(
    "cloud-run-secret-access",
    secret_id=valkey_secret.id,
    role="roles/secretmanager.secretAccessor",
    member=cloud_run_sa.email.apply(lambda email: f"serviceAccount:{email}"),
)


gcp_env = gcp.cloudrunv2.ServiceTemplateContainerEnvArgs(
    name="GOOGLE_CLOUD_PROJECT", value=gcp.config.project
)

def pick_discovery_ip(endpoints):
    for endpoint in endpoints:
        for connection in endpoint.connections:
            psc = connection.psc_auto_connection
            if psc and psc.connection_type == "CONNECTION_TYPE_DISCOVERY":
                return psc.ip_address

    raise ValueError("No PSC discovery IP was returned by GCP for the Valkey instance.")


valkey_host = valkey_instance.endpoints.apply(pick_discovery_ip)

valkey_host_env = gcp.cloudrunv2.ServiceTemplateContainerEnvArgs(
    name="VALKEY_HOST", value=valkey_host
)

valkey_port_env = gcp.cloudrunv2.ServiceTemplateContainerEnvArgs(
    name="VALKEY_PORT", value="6379"
)

valkey_pass_env = gcp.cloudrunv2.ServiceTemplateContainerEnvArgs(
    name="VALKEY_PASSWORD",
    value_source=gcp.cloudrunv2.ServiceTemplateContainerEnvValueSourceArgs(
        secret_key_ref=gcp.cloudrunv2.ServiceTemplateContainerEnvValueSourceSecretKeyRefArgs(
            secret=valkey_secret.secret_id,
            version="latest",  # Automatically pin to latest version
        )
    ),
)

service = gcp.cloudrunv2.Service(
    "c31h64-twt-axum-demo-hw-service",
    location=LOCATION,
    # ingress="INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER",
    ingress="INGRESS_TRAFFIC_ALL",
    deletion_protection=False,
    template=gcp.cloudrunv2.ServiceTemplateArgs(
        service_account=cloud_run_sa.email,
        vpc_access=gcp.cloudrunv2.ServiceTemplateVpcAccessArgs(
            egress="ALL_TRAFFIC",
            network_interfaces=[
                gcp.cloudrunv2.ServiceTemplateVpcAccessNetworkInterfaceArgs(
                    network=app_vpc.id,
                    subnetwork=app_subnet.id,
                )
            ],
        ),
        containers=[
            gcp.cloudrunv2.ServiceTemplateContainerArgs(
                image=image_name,
                envs=[gcp_env, valkey_host_env, valkey_port_env, valkey_pass_env],
                startup_probe=startup_probe,
                liveness_probe=liveness_probe,
            )
        ],
    ),
    opts=pulumi.ResourceOptions(
        depends_on=[
            frontend_bucket_sync,
            push_image,
            cloud_run_sa,
            valkey_instance,
        ]
    ),
)

global_security_policy = gcp.compute.SecurityPolicy(
    "global-security-policy",
    description="WAF and rate limiting for API",
    type="CLOUD_ARMOR",
)


rate_limit_rule = gcp.compute.SecurityPolicyRule(
    "rate-limit-rule",
    security_policy=global_security_policy.name,
    priority=1000,
    action="throttle",
    match={
        "versioned_expr": "SRC_IPS_V1",
        "config": {
            "src_ip_ranges": ["*"],  # This matches EVERYTHING
        },
    },
    rate_limit_options={
        "rate_limit_threshold": {
            "count": 5,
            "interval_sec": 60,
        },
        "conform_action": "allow",
        "exceed_action": "deny(429)",
        "enforce_on_key": "IP",
    },
)

api_neg = gcp.compute.RegionNetworkEndpointGroup(
    "api-neg",
    region=LOCATION,
    cloud_run=gcp.compute.RegionNetworkEndpointGroupCloudRunArgs(service=service.name),
)

api_backend = gcp.compute.BackendService(
    "api-backend",
    load_balancing_scheme="EXTERNAL",
    security_policy=global_security_policy.id,
    backends=[gcp.compute.BackendServiceBackendArgs(group=api_neg.id)],
)

url_map = gcp.compute.URLMap(
    "url-map",
    default_service=website_backend.id,
    path_matchers=[
        {
            "name": "routing-matcher",
            "default_service": website_backend.id,
            "path_rules": [
                {
                    "paths": ["/quote", "/adjudicate"],
                    "service": api_backend.id,  # API path goes to Cloud Run
                }
            ],
        }
    ],
    host_rules=[
        {
            "hosts": ["*"],
            "path_matcher": "routing-matcher",
        }
    ],
)

global_ip = gcp.compute.GlobalAddress("twt-global-ip", name="c31h64-twt-global-ip")

target_proxy = gcp.compute.TargetHttpProxy("http-proxy", url_map=url_map.id)

forwarding_rule = gcp.compute.GlobalForwardingRule(
    "http-rule", target=target_proxy.id, ip_address=global_ip.address, port_range="80"
)

# iam_backend = gcp.cloudrun.IamMember(
#      "c31h64-twt-public-access",
#      service = service.name,
#      location = service.location,
#      role = "roles/run.invoker",
#      member = "allUsers"
# )

iam_frontend = gcp.storage.BucketIAMMember(
    "public-bucket-access",
    bucket=frontend_bucket.name,
    role="roles/storage.objectViewer",
    member="allUsers",
)

invalidate_cdn_cache = command.local.Command(
    "invalidate-cdn-cache",
    create=url_map.name.apply(
        lambda name: f"gcloud compute url-maps invalidate-cdn-cache {name} --path '/*'"
    ),
    triggers=[frontend_bucket_sync.urn],
    opts=pulumi.ResourceOptions(
        depends_on=[frontend_bucket_sync, url_map, forwarding_rule]
    ),
)

pulumi.export("url", service.uri)
pulumi.export("load_balancer_ip", global_ip.address)
pulumi.export("valkey_host", valkey_host)
pulumi.export("valkey_port", "6379")
