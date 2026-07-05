# Secret Manager resource SHELLS only. No versions here, ever: values are added
# out-of-band (`gcloud secrets versions add <id> --data-file=-`, runbook bootstrap
# step 5) so no secret value ever touches the repo, terraform inputs, or CI logs.
# State never contains values either — only the shells.
resource "google_secret_manager_secret" "app" {
  for_each = var.secrets

  secret_id = each.value

  replication {
    auto {}
  }

  labels = {
    app        = "govfolio"
    managed-by = "terraform"
  }

  depends_on = [google_project_service.required]
}
