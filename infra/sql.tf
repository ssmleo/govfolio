# Cloud SQL Postgres 16 — single source of truth for Silver/Gold (design §3).
# PITR on (guardrail: prod migrations require a pre-apply snapshot; PITR makes every
# window recoverable). Access model: NO authorized networks — reachable only through
# the Cloud SQL connector/proxy with IAM database authentication (no passwords in
# terraform, state, or Secret Manager bootstrap). Scale notes on var.db_tier.
resource "google_sql_database_instance" "main" {
  name             = "govfolio-pg"
  database_version = "POSTGRES_16"
  region           = var.region

  settings {
    tier              = var.db_tier
    edition           = "ENTERPRISE"
    availability_type = "ZONAL" # scale note: REGIONAL when paid alert SLAs exist
    disk_size         = var.db_disk_size_gb
    disk_autoresize   = true

    ip_configuration {
      ipv4_enabled = true # no authorized_networks: connector/proxy-only access
      ssl_mode     = "ENCRYPTED_ONLY"
    }

    backup_configuration {
      enabled                        = true
      point_in_time_recovery_enabled = true
      transaction_log_retention_days = 7
      backup_retention_settings {
        retained_backups = 7
      }
    }

    database_flags {
      name  = "cloudsql.iam_authentication"
      value = "on"
    }

    maintenance_window {
      day  = 7 # Sunday
      hour = 7 # 07:00 UTC — outside US filing publication windows
    }

    user_labels = {
      app        = "govfolio"
      managed-by = "terraform"
    }
  }

  # Gold is immutable history (invariant 1) — the instance must never be a casual destroy.
  deletion_protection = true
  lifecycle {
    prevent_destroy = true
  }

  depends_on = [google_project_service.required]
}

resource "google_sql_database" "govfolio" {
  name     = "govfolio"
  instance = google_sql_database_instance.main.name
}

# IAM database auth for the runtime service accounts: no DB passwords anywhere.
# (Cloud SQL IAM SA users are named without the .gserviceaccount.com suffix.)
resource "google_sql_user" "iam_service_accounts" {
  for_each = {
    api    = google_service_account.api.email
    worker = google_service_account.worker.email
  }

  instance = google_sql_database_instance.main.name
  name     = trimsuffix(each.value, ".gserviceaccount.com")
  type     = "CLOUD_IAM_SERVICE_ACCOUNT"
}
