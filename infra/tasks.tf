# One Cloud Tasks queue per pipeline stage: discover -> fetch -> parse -> normalize ->
# publish (design §5.2). At-least-once delivery; dedup is ours (sha256 / external_id /
# fingerprint, ON CONFLICT DO NOTHING). Retries with backoff per §5.6; tasks that
# exhaust max_attempts surface via pipeline_run audit stats + review_task (fail closed),
# since Cloud Tasks has no native dead-letter queue.
resource "google_cloud_tasks_queue" "stages" {
  for_each = var.pipeline_queues

  name     = "govfolio-${each.key}"
  location = var.region

  rate_limits {
    max_concurrent_dispatches = each.value.max_concurrent
    max_dispatches_per_second = each.value.dispatches_per_second
  }

  retry_config {
    max_attempts  = 5
    min_backoff   = "10s"
    max_backoff   = "600s"
    max_doublings = 4
  }

  depends_on = [google_project_service.required]
}
