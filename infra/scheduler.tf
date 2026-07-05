# Discover cadence stubs per design §5.5 tiers. ALL PAUSED at creation: unpausing a
# tier is the explicit go-live act once its adapters pass conformance (fail closed —
# nothing polls a source before an adapter exists). Jobs call the worker's discover
# endpoint with an OIDC token minted for the invoker SA (iam.tf grants it run.invoker
# on worker only).
resource "google_cloud_scheduler_job" "discover" {
  for_each = var.discover_tiers

  name        = "govfolio-discover-${each.key}"
  description = each.value.description
  region      = var.region
  schedule    = each.value.schedule
  time_zone   = "Etc/UTC"
  paused      = true

  http_target {
    http_method = "POST"
    uri         = "${google_cloud_run_v2_service.services["worker"].uri}/stages/discover?tier=${trimprefix(each.key, "tier")}"

    oidc_token {
      service_account_email = google_service_account.invoker.email
      audience              = google_cloud_run_v2_service.services["worker"].uri
    }
  }

  retry_config {
    retry_count = 1
  }

  depends_on = [google_project_service.required]
}
