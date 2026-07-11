use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};

use crate::model::{NormalizedResult, ResultClass};

const MAX_JITTER_SECONDS: u16 = 60;

/// Supplies wall-clock time to retry policy without coupling tests to real time.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetryAction {
    Complete,
    RetryAt(DateTime<Utc>),
    UntilFingerprintChanges,
    Reconcile,
    Recover,
    Never,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyDecision {
    pub action: RetryAction,
}

impl PolicyDecision {
    #[must_use]
    pub fn retry_at(&self) -> Option<DateTime<Utc>> {
        match &self.action {
            RetryAction::RetryAt(retry_at) => Some(*retry_at),
            _ => None,
        }
    }

    #[must_use]
    pub fn disables_until_fingerprint_change(&self) -> bool {
        matches!(&self.action, RetryAction::UntilFingerprintChanges)
    }

    #[must_use]
    pub fn opens_timed_provider_circuit(&self) -> bool {
        matches!(&self.action, RetryAction::RetryAt(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StormThresholds {
    pub fingerprint_failures: i64,
    pub provider_failures: i64,
    pub system_failures: i64,
    pub window: Duration,
    pub fingerprint_cooldown: Duration,
    pub provider_cooldown: Duration,
    pub quiet_period: Duration,
}

impl Default for StormThresholds {
    fn default() -> Self {
        Self {
            fingerprint_failures: 3,
            provider_failures: 5,
            system_failures: 10,
            window: Duration::minutes(10),
            fingerprint_cooldown: Duration::hours(1),
            provider_cooldown: Duration::minutes(15),
            quiet_period: Duration::minutes(15),
        }
    }
}

pub struct PolicyEngine<C> {
    clock: C,
}

impl<C> PolicyEngine<C>
where
    C: Clock,
{
    #[must_use]
    pub const fn new(clock: C) -> Self {
        Self { clock }
    }

    #[must_use]
    pub fn decide(
        &self,
        result: &NormalizedResult,
        consecutive_failures: u32,
        fingerprint: &str,
    ) -> PolicyDecision {
        self.decide_at(result, consecutive_failures, fingerprint, self.clock.now())
    }

    #[must_use]
    pub fn decide_at(
        &self,
        result: &NormalizedResult,
        consecutive_failures: u32,
        fingerprint: &str,
        now: DateTime<Utc>,
    ) -> PolicyDecision {
        let consecutive_failures = consecutive_failures.max(1);
        let action = match result.class {
            ResultClass::Completed => RetryAction::Complete,
            ResultClass::OperatorStop => RetryAction::Never,
            ResultClass::QuotaExhausted => {
                quota_retry(now, result.retry_at, consecutive_failures, fingerprint)
            }
            ResultClass::RateLimited => {
                rate_limit_retry(now, result.retry_at, consecutive_failures)
            }
            ResultClass::TransientTransport | ResultClass::ProviderUnavailable => {
                RetryAction::RetryAt(now + transport_delay(consecutive_failures))
            }
            ResultClass::Auth
            | ResultClass::RunnerConfig
            | ResultClass::Policy
            | ResultClass::SpawnFailed => RetryAction::UntilFingerprintChanges,
            ResultClass::SessionInvalid | ResultClass::Ambiguous => RetryAction::Reconcile,
            ResultClass::PostconditionFailed => RetryAction::Recover,
        };
        PolicyDecision { action }
    }
}

fn quota_retry(
    now: DateTime<Utc>,
    reset_at: Option<DateTime<Utc>>,
    consecutive_failures: u32,
    fingerprint: &str,
) -> RetryAction {
    let retry_at = if let Some(reset_at) = reset_at {
        reset_at.max(now) + deterministic_jitter(fingerprint)
    } else {
        now + quota_delay(consecutive_failures)
    };
    RetryAction::RetryAt(retry_at)
}

fn quota_delay(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(5);
    let hours = (1_i64 << exponent).min(24);
    Duration::hours(hours)
}

fn rate_limit_retry(
    now: DateTime<Utc>,
    retry_after: Option<DateTime<Utc>>,
    consecutive_failures: u32,
) -> RetryAction {
    let cap = now + Duration::minutes(15);
    let retry_at = retry_after
        .filter(|retry_at| *retry_at > now)
        .unwrap_or_else(|| now + exponential_rate_delay(consecutive_failures))
        .min(cap);
    RetryAction::RetryAt(retry_at)
}

fn exponential_rate_delay(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(5);
    let seconds = 30_i64.saturating_mul(1_i64 << exponent).min(15 * 60);
    Duration::seconds(seconds)
}

fn transport_delay(consecutive_failures: u32) -> Duration {
    let seconds = match consecutive_failures {
        1 => 30,
        2 => 2 * 60,
        3 => 10 * 60,
        _ => 15 * 60,
    };
    Duration::seconds(seconds)
}

fn deterministic_jitter(fingerprint: &str) -> Duration {
    let digest = Sha256::digest(fingerprint.as_bytes());
    let value = u16::from_be_bytes([digest[0], digest[1]]);
    Duration::seconds(i64::from(value % MAX_JITTER_SECONDS))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{DateTime, Duration, TimeZone, Utc};

    use super::*;
    use crate::model::{NormalizedResult, ResultClass};

    struct TestClock(Mutex<DateTime<Utc>>);

    impl TestClock {
        fn at(now: DateTime<Utc>) -> Self {
            Self(Mutex::new(now))
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> DateTime<Utc> {
            *self
                .0
                .lock()
                .expect("test clock lock should not be poisoned")
        }
    }

    fn at(second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 12, 0, second)
            .single()
            .expect("test timestamp should be valid")
    }

    fn result(class: ResultClass) -> NormalizedResult {
        NormalizedResult {
            class,
            terminal_type: None,
            structured_started: true,
            session_id: None,
            provider_error_code: None,
            stable_error_hash: Some("stable".to_owned()),
            retry_at: None,
            exit_code: Some(1),
            summary: "fixture".to_owned(),
        }
    }

    #[test]
    fn quota_without_reset_should_double_from_one_hour_and_cap_at_twenty_four_hours() {
        let engine = PolicyEngine::new(TestClock::at(at(0)));
        let failures = [1, 2, 3, 4, 5, 6, 20];
        let expected_hours = [1, 2, 4, 8, 16, 24, 24];

        for (consecutive, expected) in failures.into_iter().zip(expected_hours) {
            let decision = engine.decide(&result(ResultClass::QuotaExhausted), consecutive, "q");
            assert_eq!(
                decision.action,
                RetryAction::RetryAt(at(0) + Duration::hours(expected))
            );
        }
    }

    #[test]
    fn quota_reset_should_add_stable_bounded_jitter() {
        let engine = PolicyEngine::new(TestClock::at(at(0)));
        let mut outcome = result(ResultClass::QuotaExhausted);
        outcome.retry_at = Some(at(0) + Duration::hours(3));

        let first = engine.decide(&outcome, 1, "same-fingerprint");
        let second = engine.decide(&outcome, 99, "same-fingerprint");

        assert_eq!(first, second);
        let RetryAction::RetryAt(retry_at) = first.action else {
            panic!("quota reset should produce a timed retry")
        };
        assert!(retry_at >= outcome.retry_at.expect("fixture has retry time"));
        assert!(
            retry_at < outcome.retry_at.expect("fixture has retry time") + Duration::minutes(1)
        );
    }

    #[test]
    fn transport_should_use_exact_sequence_and_cap() {
        let engine = PolicyEngine::new(TestClock::at(at(0)));
        let failures = [1, 2, 3, 4, 10];
        let expected = [30, 120, 600, 900, 900];

        for (consecutive, seconds) in failures.into_iter().zip(expected) {
            let decision = engine.decide(
                &result(ResultClass::TransientTransport),
                consecutive,
                "transport",
            );
            assert_eq!(
                decision.action,
                RetryAction::RetryAt(at(0) + Duration::seconds(seconds))
            );
        }
    }

    #[test]
    fn rate_limit_should_honor_retry_after_but_cap_at_fifteen_minutes() {
        let engine = PolicyEngine::new(TestClock::at(at(0)));
        let mut outcome = result(ResultClass::RateLimited);
        outcome.retry_at = Some(at(0) + Duration::hours(2));

        let decision = engine.decide(&outcome, 1, "rate");

        assert_eq!(
            decision.action,
            RetryAction::RetryAt(at(0) + Duration::minutes(15))
        );
    }

    #[test]
    fn deterministic_classes_should_not_immediately_retry() {
        let engine = PolicyEngine::new(TestClock::at(at(0)));

        assert_eq!(
            engine.decide(&result(ResultClass::Auth), 1, "auth").action,
            RetryAction::UntilFingerprintChanges
        );
        assert_eq!(
            engine
                .decide(&result(ResultClass::OperatorStop), 1, "stop")
                .action,
            RetryAction::Never
        );
        assert_eq!(
            engine
                .decide(&result(ResultClass::Ambiguous), 1, "ambiguous")
                .action,
            RetryAction::Reconcile
        );
        assert_eq!(
            engine
                .decide(&result(ResultClass::PostconditionFailed), 1, "dirty")
                .action,
            RetryAction::Recover
        );
    }

    #[test]
    fn storm_thresholds_should_match_release_zero_defaults() {
        assert_eq!(StormThresholds::default().fingerprint_failures, 3);
        assert_eq!(StormThresholds::default().provider_failures, 5);
        assert_eq!(StormThresholds::default().system_failures, 10);
        assert_eq!(StormThresholds::default().window, Duration::minutes(10));
        assert_eq!(
            StormThresholds::default().quiet_period,
            Duration::minutes(15)
        );
    }
}
