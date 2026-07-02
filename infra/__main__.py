"""A Google Cloud Python Pulumi program"""

import pulumi
import pulumi_gcp as gcp
import pulumi_command as command

repo = gcp.artifactregistry.Repository(
    "c31h64-threewhitetowers-repo",
    location="europe-west1",
    repository_id="c31h64-twt-repo",
    format="DOCKER"
)

push_image = command.local.Command("push-docker-image",
                                   create="just push",
                                   triggers=[pulumi.asset.FileArchive("../app")],
                                   opts=pulumi.ResourceOptions(depends_on=[repo]))

image_name = repo.name.apply(
    lambda name: f"europe-west1-docker.pkg.dev/{gcp.config.project}/c31h64-twt-repo/axum-demo-hw:latest"
)

startup_probe = gcp.cloudrun.ServiceTemplateSpecContainerStartupProbeArgs(
    http_get=gcp.cloudrun.ServiceTemplateSpecContainerStartupProbeHttpGetArgs(
        path="/ready"    
    ),
    initial_delay_seconds=1,
    timeout_seconds=15,
    failure_threshold=3,
    period_seconds=15
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

gcl_env = gcp.cloudrun.ServiceTemplateSpecContainerEnvArgs(
    name="GOOGLE_CLOUD_LOCATION",
    value="global"
)

service = gcp.cloudrun.Service(
    "c31h64-twt-axum-demo-hw-service",
    location="europe-west1",
    template=gcp.cloudrun.ServiceTemplateArgs(
        spec=gcp.cloudrun.ServiceTemplateSpecArgs(
            service_account_name=gemini_sa.email,
            containers=[
                gcp.cloudrun.ServiceTemplateSpecContainerArgs(
                    image=image_name,
                    envs=[gcp_env, gcl_env],
                    startup_probe=startup_probe,
                    liveness_probe=liveness_probe
                )
            ]
        )
    ),
    opts=pulumi.ResourceOptions(depends_on=[push_image, gemini_sa])
)


iam = gcp.cloudrun.IamMember(
    "c31h64-twt-public-access",
    service = service.name,
    location = service.location,
    role = "roles/run.invoker",
    member = "allUsers"
)

pulumi.export("url", service.statuses.apply(lambda s: s[0].url))
pulumi.export("sa_email", gemini_sa.email)
