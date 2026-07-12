//! Host-local supervision for govfolio's unattended Claude/Codex loops.
//!
//! The crate deliberately owns process/control concerns only. Product-domain
//! integration receipts remain in Postgres through `govfolio_core`.

// The supervisor is an internal binary crate. Its public surface exists only so
// the CLI and protocol tests can share implementation; user-facing failure
// context is added at the command boundary rather than repeated on every helper.
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::unused_self)]
#![cfg_attr(test, allow(clippy::expect_used))]

pub mod artifacts;
pub mod build_classifier;
pub mod build_interference;
pub mod build_policy;
pub mod build_protocol;
pub mod build_scheduler;
pub mod build_service;
pub mod build_shim;
pub mod build_store;
pub mod build_transport;
pub mod canary;
pub mod config;
pub mod failover;
pub mod historical_contract;
pub mod host;
pub mod integration;
pub mod model;
pub mod policy;
pub mod preflight;
pub mod process;
pub mod provider;
pub mod store;
pub mod supervisor;
