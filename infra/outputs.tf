output "cloud_run_urls" {
  description = "Service URLs (point Cloudflare DNS at api/web; worker is IAM-only)."
  value       = { for k, s in google_cloud_run_v2_service.services : k => s.uri }
}

output "sql_connection_name" {
  description = "Cloud SQL connection name for the connector/proxy (project:region:instance)."
  value       = google_sql_database_instance.main.connection_name
}

output "buckets" {
  description = "GCS bucket names."
  value = {
    bronze  = google_storage_bucket.bronze.name
    exports = google_storage_bucket.exports.name
  }
}

output "artifact_repository" {
  description = "Docker push prefix for the deploy workflow."
  value       = "${var.region}-docker.pkg.dev/${var.project_id}/${google_artifact_registry_repository.images.repository_id}"
}

output "wif_provider" {
  description = "Set as GitHub repo variable GCP_WIF_PROVIDER."
  value       = google_iam_workload_identity_pool_provider.github_actions.name
}

output "deploy_service_account" {
  description = "Set as GitHub repo variable GCP_DEPLOY_SA."
  value       = google_service_account.deploy.email
}

output "terraform_service_account" {
  description = "Set as GitHub repo variable GCP_TF_SA."
  value       = google_service_account.terraform.email
}
