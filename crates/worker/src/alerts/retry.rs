//! Retry with exponential backoff, counting attempts — the counter is
//! persisted on the delivery row, so it must be visible to the caller
//! (unlike the pipeline's `with_backoff`, which hides it).

use std::time::Duration;

use crate::alerts::SendError;

/// Runs `op` up to `max_attempts` times, sleeping `base * 2^n` between
/// attempts. Non-retryable [`SendError`]s (and non-`SendError` failures)
/// stop immediately — retrying a 4xx or a malformed address cannot help.
///
/// Returns `Ok(attempts_made)` on success, `Err((attempts_made, error))` on
/// final failure.
///
/// # Errors
/// The final attempt's error, with the attempt count.
pub async fn send_with_retry<F, Fut>(
    max_attempts: u32,
    base: Duration,
    op: F,
) -> Result<u32, (u32, anyhow::Error)>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let max_attempts = max_attempts.max(1);
    let mut attempts = 0u32;
    loop {
        attempts += 1;
        match op().await {
            Ok(()) => return Ok(attempts),
            Err(error) => {
                let retryable = error
                    .downcast_ref::<SendError>()
                    .is_some_and(|e| e.retryable);
                if !retryable || attempts >= max_attempts {
                    return Err((attempts, error));
                }
                tokio::time::sleep(base * 2u32.saturating_pow(attempts - 1)).await;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn retryable_failure() -> anyhow::Error {
        SendError {
            retryable: true,
            message: "503 (test)".to_owned(),
        }
        .into()
    }

    #[tokio::test(start_paused = true)]
    async fn alerts_retry_backs_off_exponentially_then_gives_up() {
        let started = tokio::time::Instant::now();
        let calls = AtomicU32::new(0);
        let result = send_with_retry(3, Duration::from_millis(100), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err(retryable_failure()) }
        })
        .await;
        let (attempts, error) = result.unwrap_err();
        assert_eq!(attempts, 3);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert!(error.to_string().contains("503"));
        // 100ms + 200ms of backoff elapsed (paused clock auto-advances).
        assert_eq!(started.elapsed(), Duration::from_millis(300));
    }

    #[tokio::test(start_paused = true)]
    async fn alerts_retry_succeeds_midway_and_reports_attempts() {
        let calls = AtomicU32::new(0);
        let attempts = send_with_retry(5, Duration::from_millis(1), || {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            async move {
                if n < 2 {
                    Err(retryable_failure())
                } else {
                    Ok(())
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(attempts, 3, "two failures + the success");
    }

    #[tokio::test(start_paused = true)]
    async fn alerts_retry_stops_immediately_on_terminal_errors() {
        let started = tokio::time::Instant::now();
        let result = send_with_retry(5, Duration::from_mins(1), || async {
            Err(SendError {
                retryable: false,
                message: "404 (test)".to_owned(),
            }
            .into())
        })
        .await;
        let (attempts, _) = result.unwrap_err();
        assert_eq!(attempts, 1, "terminal errors get no retries");
        assert_eq!(started.elapsed(), Duration::ZERO, "and no backoff sleeps");
    }
}
