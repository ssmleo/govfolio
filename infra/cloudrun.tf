# Three Cloud Run services, all scale-to-zero (design §3/§8): api + worker are Rust,
# web is Next.js SSR. Images are deployed by .github/workflows/deploy.yml (gcloud run
# deploy); terraform owns everything EXCEPT the image, so ignore_changes keeps the two
# from fighting. First apply uses var.bootstrap_image (public hello container).
#
# Secret/env wiring (DATABASE_URL from Secret Manager, etc.) lands with the real images:
# referencing a secret with zero versions would fail the first revision. IAM below is
# already granted so wiring is a template-only change.
locals {
  run_services = {
    api    = { service_account = google_service_account.api.email }
    web    = { service_account = google_service_account.web.email }
    worker = { service_account = google_service_account.worker.email }
  }
}

resource "google_cloud_run_v2_service" "services" {
  for_each = local.run_services

  name     = "govfolio-${each.key}"
  location = var.region
  ingress  = "INGRESS_TRAFFIC_ALL" # worker stays private via IAM (no public invoker)

  template {
    service_account = each.value.service_account

    scaling {
      min_instance_count = 0
      max_instance_count = var.service_max_instances[each.key]
    }

    containers {
      image = var.bootstrap_image

      resources {
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
        cpu_idle = true # request-based billing — pay only while serving
      }

      ports {
        container_port = 8080
      }
    }
  }

  labels = {
    app        = "govfolio"
    managed-by = "terraform"
  }

  deletion_protection = true

  lifecycle {
    ignore_changes = [
      template[0].containers[0].image,
      client,
      client_version,
    ]
  }

  depends_on = [google_project_service.required]
}

# api + web are public products (Cloudflare fronts DNS/CDN/WAF + coarse rate limits,
# design §8). worker is IAM-only: invoked by Cloud Scheduler / Cloud Tasks via the
# invoker service account (iam.tf) — never by the public.
resource "google_cloud_run_v2_service_iam_member" "public" {
  for_each = toset(["api", "web"])

  name     = google_cloud_run_v2_service.services[each.value].name
  location = var.region
  role     = "roles/run.invoker"
  member   = "allUsers"
}
