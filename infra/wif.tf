# Workload Identity Federation: GitHub Actions deploys with short-lived tokens —
# NO service-account keys, committed or otherwise (runbook non-negotiable). Only
# workflows from var.github_repository can impersonate the deploy SA.
# After first apply, set the GitHub repo variables (values from outputs.tf):
#   GCP_WIF_PROVIDER, GCP_DEPLOY_SA, GCP_TF_SA, GCP_STATE_BUCKET, GCP_PROJECT_ID
resource "google_iam_workload_identity_pool" "github" {
  workload_identity_pool_id = "github"
  display_name              = "GitHub Actions"

  depends_on = [google_project_service.required]
}

resource "google_iam_workload_identity_pool_provider" "github_actions" {
  workload_identity_pool_id          = google_iam_workload_identity_pool.github.workload_identity_pool_id
  workload_identity_pool_provider_id = "github-actions"
  display_name                       = "GitHub Actions OIDC"

  attribute_mapping = {
    "google.subject"       = "assertion.sub"
    "attribute.repository" = "assertion.repository"
    "attribute.ref"        = "assertion.ref"
  }

  # Fail closed: tokens from any other repository are rejected at the pool boundary.
  attribute_condition = "assertion.repository == \"${var.github_repository}\""

  oidc {
    issuer_uri = "https://token.actions.githubusercontent.com"
  }
}

resource "google_service_account_iam_member" "ci_wif" {
  for_each = {
    deploy    = google_service_account.deploy.name
    terraform = google_service_account.terraform.name
  }

  service_account_id = each.value
  role               = "roles/iam.workloadIdentityUser"
  member             = "principalSet://iam.googleapis.com/${google_iam_workload_identity_pool.github.name}/attribute.repository/${var.github_repository}"
}
