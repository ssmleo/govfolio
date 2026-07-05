# Least-privilege service accounts (runbook non-negotiable: exact scopes, no owner
# roles, no keys — WIF only for CI). One SA per runtime service + invoker + deploy.

resource "google_service_account" "api" {
  account_id   = "govfolio-api"
  display_name = "govfolio api (Cloud Run runtime)"
}

resource "google_service_account" "web" {
  account_id   = "govfolio-web"
  display_name = "govfolio web (Cloud Run runtime; talks only to api over HTTP)"
}

resource "google_service_account" "worker" {
  account_id   = "govfolio-worker"
  display_name = "govfolio worker (pipeline stages via Cloud Tasks)"
}

resource "google_service_account" "invoker" {
  account_id   = "govfolio-invoker"
  display_name = "govfolio invoker (OIDC identity for Scheduler/Tasks -> worker)"
}

resource "google_service_account" "deploy" {
  account_id   = "govfolio-deploy"
  display_name = "govfolio deploy (GitHub Actions via WIF — no keys)"
}

resource "google_service_account" "terraform" {
  account_id   = "govfolio-terraform"
  display_name = "govfolio terraform (CI plan/apply via WIF — no keys, no owner)"
}

# --- database access (Cloud SQL connector + IAM db auth; no passwords) ---------------

resource "google_project_iam_member" "cloudsql_client" {
  for_each = {
    api    = google_service_account.api.email
    worker = google_service_account.worker.email
  }

  project = var.project_id
  role    = "roles/cloudsql.client"
  member  = "serviceAccount:${each.value}"
}

# --- secrets (per-secret grants, not project-wide) ------------------------------------

resource "google_secret_manager_secret_iam_member" "api_database_url" {
  secret_id = google_secret_manager_secret.app["database-url"].secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.api.email}"
}

resource "google_secret_manager_secret_iam_member" "worker_secrets" {
  for_each = var.secrets

  secret_id = google_secret_manager_secret.app[each.value].secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.worker.email}"
}

# --- buckets (bucket-level, not project-level) ----------------------------------------

# worker writes Bronze (immutable puts; versioning + no delete role keeps raw sacred)
# and exports.
resource "google_storage_bucket_iam_member" "worker_bronze" {
  bucket = google_storage_bucket.bronze.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.worker.email}"
}

resource "google_storage_bucket_iam_member" "worker_exports" {
  bucket = google_storage_bucket.exports.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.worker.email}"
}

# api reads Bronze to mint signed URLs (design §8); signing via IAM signBlob needs
# tokenCreator on its OWN identity only.
resource "google_storage_bucket_iam_member" "api_bronze_read" {
  bucket = google_storage_bucket.bronze.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.api.email}"
}

resource "google_service_account_iam_member" "api_self_sign" {
  service_account_id = google_service_account.api.name
  role               = "roles/iam.serviceAccountTokenCreator"
  member             = "serviceAccount:${google_service_account.api.email}"
}

# --- pipeline plumbing -----------------------------------------------------------------

# worker enqueues the next stage (per-queue, not project-wide) ...
resource "google_cloud_tasks_queue_iam_member" "worker_enqueue" {
  for_each = var.pipeline_queues

  name     = google_cloud_tasks_queue.stages[each.key].name
  location = var.region
  role     = "roles/cloudtasks.enqueuer"
  member   = "serviceAccount:${google_service_account.worker.email}"
}

# ... and must be able to attach the invoker SA's OIDC identity to those tasks.
resource "google_service_account_iam_member" "worker_acts_as_invoker" {
  service_account_id = google_service_account.invoker.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.worker.email}"
}

# Scheduler/Tasks (as invoker SA) may call ONLY the worker service.
resource "google_cloud_run_v2_service_iam_member" "invoker_worker" {
  name     = google_cloud_run_v2_service.services["worker"].name
  location = var.region
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.invoker.email}"
}

# --- deploy (GitHub Actions) -------------------------------------------------------------

# Per-service deploy rights (new revisions), not project-wide run.admin.
resource "google_cloud_run_v2_service_iam_member" "deploy_developer" {
  for_each = local.run_services

  name     = google_cloud_run_v2_service.services[each.key].name
  location = var.region
  role     = "roles/run.developer"
  member   = "serviceAccount:${google_service_account.deploy.email}"
}

# Deploying a revision that runs AS a runtime SA requires actAs on that SA.
resource "google_service_account_iam_member" "deploy_acts_as_runtime" {
  for_each = local.run_services

  service_account_id = "projects/${var.project_id}/serviceAccounts/${each.value.service_account}"
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.deploy.email}"
}

resource "google_artifact_registry_repository_iam_member" "deploy_push" {
  repository = google_artifact_registry_repository.images.name
  location   = var.region
  role       = "roles/artifactregistry.writer"
  member     = "serviceAccount:${google_service_account.deploy.email}"
}

# --- terraform CI apply (deploy.yml `terraform` job) -----------------------------------
#
# Applying this stack needs admin over exactly the resource types it manages — project-
# scoped, enumerated, NO roles/owner or roles/editor (runbook non-negotiable). The two
# powerful grants are justified and bounded:
#   * projectIamAdmin — required for google_project_iam_member (cloudsql.client) bindings;
#     caller is WIF-locked to var.github_repository, applies only run pre-gated plans
#     (check-tf-plan.sh), and remote state is versioned (every apply recoverable).
#   * serviceAccountAdmin — required to manage the runtime SAs declared here.
resource "google_project_iam_member" "terraform_admin" {
  for_each = toset([
    "roles/artifactregistry.admin",
    "roles/cloudscheduler.admin",
    "roles/cloudsql.admin",
    "roles/cloudtasks.admin",
    "roles/iam.serviceAccountAdmin",
    "roles/iam.workloadIdentityPoolAdmin",
    "roles/resourcemanager.projectIamAdmin",
    "roles/run.admin",
    "roles/secretmanager.admin",
    "roles/serviceusage.serviceUsageAdmin",
    "roles/storage.admin",
  ])

  project = var.project_id
  role    = each.value
  member  = "serviceAccount:${google_service_account.terraform.email}"
}

# actAs scoped to the SAs this stack attaches to resources (Cloud Run templates,
# Scheduler OIDC) — NOT project-wide serviceAccountUser.
resource "google_service_account_iam_member" "terraform_acts_as" {
  for_each = {
    api     = google_service_account.api.name
    web     = google_service_account.web.name
    worker  = google_service_account.worker.name
    invoker = google_service_account.invoker.name
  }

  service_account_id = each.value
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.terraform.email}"
}
