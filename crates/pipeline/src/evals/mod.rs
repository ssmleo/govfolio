//! Role-eval harness + epoch gate (goal 016).
//!
//! Deterministic, MECHANICAL scorers — no LLM judge anywhere
//! (world-verifies-model; `docs/decisions/automation-policy.md`) — score each
//! role's E1 artifact against the frozen `us_house` reference bundle pinned by
//! [`reference::LOCK_PATH`]. Roles whose reference artifact does not exist
//! score [`Outcome::NotApplicable`], and the epoch gate treats that as
//! BLOCKING (fail closed): E2 needs those phases live, so gating on their
//! absence is correct.
//!
//! Thresholds and `NOT_APPLICABLE` gating semantics are documented in
//! `docs/decisions/role-eval-thresholds.md` (founder-gated like role edits).
//! A scorer that finds a defect in a pinned reference artifact surfaces a
//! FINDING via its check detail — it never edits the artifact (freeze =
//! supersede, never mutate).

mod reference;
mod roles;

pub use reference::{LOCK_PATH, ReferenceLock, load_lock, verify_lock};

use std::path::Path;

/// The roles the E1 calibration harness scores (coverage-factory phases,
/// `agents/workflows/source-exploration.md`). Roles outside this set
/// (orchestrator, planner, sentinel, web-builder) have no us_house-scoped
/// reference artifact yet — see the thresholds doc for the rationale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Phase 0 — `docs/regimes/us_house/sources.yaml`.
    Scout,
    /// Phase 1 — `docs/regimes/us_house/AUTHORITY.md` front-matter.
    Surveyor,
    /// Phase 2 — fixture capture manifest attributed to a sampler run.
    Sampler,
    /// Phase 3 — the regime doc (`docs/regimes/us-house.md`).
    SpecWriter,
    /// Phase 3 — fixtures + manifest + `expected.*.json` contracts.
    TestDesigner,
    /// Phase 4 — frozen conformance corpus + recorded calibration evidence.
    RustBuilder,
    /// Cross-phase — audit journal line + goal-file findings sections.
    Auditor,
}

impl Role {
    /// Every scored role, in coverage-factory phase order.
    pub const ALL: [Self; 7] = [
        Self::Scout,
        Self::Surveyor,
        Self::Sampler,
        Self::SpecWriter,
        Self::TestDesigner,
        Self::RustBuilder,
        Self::Auditor,
    ];

    /// Role name as used in `agents/roles/<name>.md`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scout => "scout",
            Self::Surveyor => "surveyor",
            Self::Sampler => "sampler",
            Self::SpecWriter => "spec-writer",
            Self::TestDesigner => "test-designer",
            Self::RustBuilder => "rust-builder",
            Self::Auditor => "auditor",
        }
    }

    /// Minimum passing score. Conservative default: 1.00 for every role —
    /// each check is mechanical and runs against a known-good, audited
    /// reference, so anything below full marks is drift or a defect.
    /// Mirrored (with rationale) in `docs/decisions/role-eval-thresholds.md`;
    /// changing a threshold is founder-gated like role edits.
    #[must_use]
    pub const fn threshold(self) -> f64 {
        1.0
    }
}

/// One mechanical check inside a role score.
#[derive(Debug)]
pub struct Check {
    /// Stable check identifier.
    pub name: &'static str,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable evidence (or the finding, when failed).
    pub detail: String,
}

impl Check {
    pub(crate) fn new(name: &'static str, passed: bool, detail: impl Into<String>) -> Self {
        Self {
            name,
            passed,
            detail: detail.into(),
        }
    }
}

/// A role's eval outcome against the frozen reference.
#[derive(Debug)]
pub enum Outcome {
    /// The role has a reference artifact; score = passed checks / checks.
    Scored {
        /// The mechanical checks that make up the score.
        checks: Vec<Check>,
    },
    /// No reference artifact exists for this role — BLOCKING for epoch
    /// entry until one does (fail closed).
    NotApplicable {
        /// Why the role cannot be scored yet.
        reason: String,
    },
}

/// Fraction of passed checks; an empty check list scores 0.0 (no evidence,
/// no credit — fail closed).
#[must_use]
pub fn score(checks: &[Check]) -> f64 {
    if checks.is_empty() {
        return 0.0;
    }
    let passed = checks.iter().filter(|c| c.passed).count();
    let passed32 = u32::try_from(passed).unwrap_or(u32::MAX);
    let total32 = u32::try_from(checks.len()).unwrap_or(u32::MAX);
    f64::from(passed32) / f64::from(total32)
}

/// One role's report card.
#[derive(Debug)]
pub struct RoleReport {
    /// The role scored.
    pub role: Role,
    /// Its outcome.
    pub outcome: Outcome,
}

impl RoleReport {
    /// Score when applicable; `None` for `NOT_APPLICABLE`.
    #[must_use]
    pub fn score(&self) -> Option<f64> {
        match &self.outcome {
            Outcome::Scored { checks } => Some(score(checks)),
            Outcome::NotApplicable { .. } => None,
        }
    }

    /// True when scored and at/above the role's threshold.
    #[must_use]
    pub fn meets_threshold(&self) -> bool {
        self.score().is_some_and(|s| s >= self.role.threshold())
    }
}

/// Scores one role against the frozen E1 reference under `root`.
#[must_use]
pub fn score_role(root: &Path, role: Role) -> RoleReport {
    let outcome = match role {
        Role::Scout => roles::scout(root),
        Role::Surveyor => roles::surveyor(root),
        Role::Sampler => roles::sampler(root),
        Role::SpecWriter => roles::spec_writer(root),
        Role::TestDesigner => roles::test_designer(root),
        Role::RustBuilder => roles::rust_builder(root),
        Role::Auditor => roles::auditor(root),
    };
    RoleReport { role, outcome }
}

/// Scores every role in [`Role::ALL`] using only frozen artifacts and
/// recorded calibration evidence. Current-code verification is a separate,
/// commit-bound release gate.
#[must_use]
pub fn score_all(root: &Path) -> Vec<RoleReport> {
    Role::ALL
        .into_iter()
        .map(|role| score_role(root, role))
        .collect()
}

/// Epoch-gate report: reference-bundle integrity + per-role scores +
/// blockers. Empty `blockers` = gate open.
#[derive(Debug)]
pub struct GateReport {
    /// Epoch whose entry is being gated (only `E2` is wired).
    pub epoch: String,
    /// Reference-bundle freeze failures (tamper/drift evidence).
    pub lock_failures: Vec<String>,
    /// Per-role report cards.
    pub roles: Vec<RoleReport>,
    /// Why the gate is blocked; empty means open.
    pub blockers: Vec<String>,
}

impl GateReport {
    /// True when nothing blocks epoch entry.
    #[must_use]
    pub fn open(&self) -> bool {
        self.blockers.is_empty()
    }
}

/// Runs the epoch gate: verifies the frozen reference bundle, scores every
/// role, and blocks (fail closed) on lock drift, any applicable role below
/// threshold, or any `NOT_APPLICABLE` role (missing reference artifacts —
/// the epoch needs those phases live).
///
/// # Errors
/// An epoch other than `E2` — only the E1-reference-calibrated E2 gate is
/// wired; later gates need their own reference bundles.
pub fn gate(root: &Path, epoch: &str) -> anyhow::Result<GateReport> {
    anyhow::ensure!(
        epoch == "E2",
        "only the E2 gate is wired (calibrated against the frozen E1 us_house reference); \
         got {epoch:?} — later epoch gates need their own reference bundles (fail closed)"
    );
    let lock_failures = verify_lock(root);
    let mut blockers: Vec<String> = lock_failures
        .iter()
        .map(|f| format!("reference bundle not intact: {f}"))
        .collect();
    let roles = score_all(root);
    for report in &roles {
        match &report.outcome {
            Outcome::Scored { checks } => {
                if !report.meets_threshold() {
                    let failed: Vec<&str> = checks
                        .iter()
                        .filter(|c| !c.passed)
                        .map(|c| c.name)
                        .collect();
                    blockers.push(format!(
                        "role {} scored {:.2} < threshold {:.2} (failed checks: {})",
                        report.role.name(),
                        score(checks),
                        report.role.threshold(),
                        failed.join(", ")
                    ));
                }
            }
            Outcome::NotApplicable { reason } => {
                blockers.push(format!(
                    "role {} has no reference artifact — BLOCKING for {epoch} entry \
                     (fail closed, docs/decisions/role-eval-thresholds.md): {reason}",
                    report.role.name()
                ));
            }
        }
    }
    Ok(GateReport {
        epoch: epoch.to_owned(),
        lock_failures,
        roles,
        blockers,
    })
}
