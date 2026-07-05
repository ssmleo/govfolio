variable "project_id" {
  description = "GCP project id."
  type        = string
  default     = "govfolio"
}

variable "region" {
  # us-central1: GCP tier-1 (cheapest) pricing band, full availability of every service
  # we use (Cloud Run, Cloud SQL, Tasks, Scheduler, Artifact Registry), and US-domiciled —
  # closest to the launch jurisdictions' sources (US House/Senate, design §5.7).
  description = "Primary region for all regional resources."
  type        = string
  default     = "us-central1"
}

variable "github_repository" {
  description = "GitHub <owner>/<repo> allowed to deploy via Workload Identity Federation."
  type        = string
  default     = "ssmleo/govfolio"
}

variable "db_tier" {
  # Smallest sane tier: db-f1-micro (shared-core, ENTERPRISE edition) is fine while the
  # workload is a handful of Tier-1 adapters. Scale path (in order, all zero-downtime-ish
  # via Cloud SQL in-place edits): db-g1-small -> db-custom-1-3840 -> db-custom-2-7680.
  # Move availability_type to REGIONAL when paid alert SLAs exist (doubles cost).
  description = "Cloud SQL machine tier."
  type        = string
  default     = "db-f1-micro"
}

variable "db_disk_size_gb" {
  description = "Initial Cloud SQL disk size (GB); autoresize is on."
  type        = number
  default     = 10
}

variable "bootstrap_image" {
  # Placeholder so the substrate applies before any app image exists. Real images are
  # deployed by .github/workflows/deploy.yml; terraform ignores image drift (see cloudrun.tf).
  description = "Image used for the first Cloud Run revision only."
  type        = string
  default     = "us-docker.pkg.dev/cloudrun/container/hello"
}

variable "service_max_instances" {
  description = "Per-service Cloud Run max instances (min is always 0 — scale-to-zero)."
  type        = map(number)
  default = {
    api    = 2
    web    = 2
    worker = 3
  }
}

variable "pipeline_queues" {
  # One Cloud Tasks queue per pipeline stage (design §5.2). Egress stages (discover,
  # fetch) keep concurrency 1 / 1 rps — politeness invariant #10 is also enforced
  # in-process per source; the queue setting is the outer bound. Internal stages get
  # modest parallelism.
  description = "Per-stage queue rate limits."
  type = map(object({
    max_concurrent        = number
    dispatches_per_second = number
  }))
  default = {
    discover  = { max_concurrent = 1, dispatches_per_second = 1 }
    fetch     = { max_concurrent = 1, dispatches_per_second = 1 }
    parse     = { max_concurrent = 4, dispatches_per_second = 5 }
    normalize = { max_concurrent = 4, dispatches_per_second = 5 }
    publish   = { max_concurrent = 2, dispatches_per_second = 5 }
  }
}

variable "discover_tiers" {
  # Cadence stubs per design §5.5. All jobs start PAUSED — unpause per tier as adapters
  # go live. Tier 1 stub is 5 min; the in-window 1-min burst cadence is an app-level
  # concern (worker self-schedules inside publication windows), not a Scheduler edit.
  description = "Cloud Scheduler discover cadences per tier."
  type = map(object({
    schedule    = string
    description = string
  }))
  default = {
    tier1 = { schedule = "*/5 * * * *", description = "US House/Senate transaction reports (design 5.5 tier 1)" }
    tier2 = { schedule = "0 * * * *", description = "Change-notification registers: UK/AU/CA (tier 2)" }
    tier3 = { schedule = "0 6 * * *", description = "Annual-declaration regimes: EU-P/FR/DE/... (tier 3)" }
  }
}

variable "secrets" {
  # Resource SHELLS only — versions (values) are added out-of-band via
  # `gcloud secrets versions add` (runbook bootstrap step). No value ever lives in
  # terraform code, state inputs, or the repo.
  description = "Secret Manager secret ids to create (no versions/values)."
  type        = set(string)
  default = [
    "database-url",    # postgres connection string for api/worker
    "openfigi-api-key" # instrument resolution waterfall (design §5.4)
  ]
}
