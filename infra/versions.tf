# govfolio cloud substrate — goal 020. Design: docs/plans/2026-07-04-govfolio-design.md §3/§8.
# Runbook (bootstrap, guardrails, halt semantics): docs/runbooks/deploy.md

terraform {
  required_version = ">= 1.10"

  required_providers {
    google = {
      source = "hashicorp/google"
      # Latest stable 6.x; exact build pinned by the committed .terraform.lock.hcl.
      version = "~> 6.0"
    }
  }

  # Remote state: GCS with native state locking + object versioning (automation-policy
  # guardrail 2 — every apply recoverable). The bucket CANNOT be declared here
  # (chicken-egg: state must exist before terraform can manage it) and backend blocks
  # cannot read variables, so it is intentionally empty. Bootstrap once:
  #   gcloud storage buckets create gs://<state-bucket> --location=us-central1 \
  #     --uniform-bucket-level-access --public-access-prevention
  #   gcloud storage buckets update gs://<state-bucket> --versioning
  #   terraform -chdir=infra init -backend-config="bucket=<state-bucket>"
  # Convention: <state-bucket> = govfolio-terraform-state. See docs/runbooks/deploy.md.
  backend "gcs" {
    prefix = "infra"
  }
}

provider "google" {
  project = var.project_id
  region  = var.region
}
