# Bronze: immutable, sha256-addressed raw documents (invariant 2 — raw is sacred).
# Versioning ON + public access enforced-off + prevent_destroy: no path deletes raw.
resource "google_storage_bucket" "bronze" {
  name                        = "${var.project_id}-bronze"
  location                    = var.region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  versioning {
    enabled = true
  }

  labels = {
    app        = "govfolio"
    managed-by = "terraform"
    layer      = "bronze"
  }

  lifecycle {
    prevent_destroy = true
  }

  depends_on = [google_project_service.required]
}

# Exports: regenerable artifacts (bulk downloads, diff reports). Versioned per goal 020,
# but noncurrent versions are trimmed — unlike bronze, exports are reproducible.
resource "google_storage_bucket" "exports" {
  name                        = "${var.project_id}-exports"
  location                    = var.region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  versioning {
    enabled = true
  }

  lifecycle_rule {
    action {
      type = "Delete"
    }
    condition {
      num_newer_versions = 5
      with_state         = "ARCHIVED"
    }
  }

  labels = {
    app        = "govfolio"
    managed-by = "terraform"
    layer      = "exports"
  }

  depends_on = [google_project_service.required]
}
