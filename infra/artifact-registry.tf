# Container images for api/web/worker. gcr.io is deprecated; Artifact Registry is the
# push target for the deploy workflow: ${region}-docker.pkg.dev/${project}/govfolio/<svc>.
# Image GC (cleanup policies) deliberately deferred until real deploy volume exists.
resource "google_artifact_registry_repository" "images" {
  repository_id = "govfolio"
  location      = var.region
  format        = "DOCKER"
  description   = "govfolio service images (api, web, worker)"

  labels = {
    app        = "govfolio"
    managed-by = "terraform"
  }

  depends_on = [google_project_service.required]
}
