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

service = gcp.cloudrun.Service(
    "c31h64-twt-axum-demo-hw-service",
    location="europe-west1",
    template=gcp.cloudrun.ServiceTemplateArgs(
        spec=gcp.cloudrun.ServiceTemplateSpecArgs(
            containers=[
                gcp.cloudrun.ServiceTemplateSpecContainerArgs(
                    image=image_name,
                )
            ]
        )
    ),
    opts=pulumi.ResourceOptions(depends_on=[push_image])
)


iam = gcp.cloudrun.IamMember(
    "c31h64-twt-public-access",
    service = service.name,
    location = service.location,
    role = "roles/run.invoker",
    member = "allUsers"
)

pulumi.export("url", service.statuses.apply(lambda s: s[0].url))
