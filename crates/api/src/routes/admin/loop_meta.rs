//! `GET /v1/admin/loop` — section H, autonomous-loop meta (repo-root-gated):
//! goal queue parsed from `agents/goals/000-INDEX.md` with HALT detection
//! (H1), git activity via subprocess with git-failure → `git: null` (H2),
//! guardrail trips from `agents/JOURNAL.md` (H3). When
//! [`crate::ApiConfig::repo_root`] is `None` (the cloud posture) the
//! endpoint answers 503 Unavailable by design.
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
