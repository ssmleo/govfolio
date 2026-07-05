//! Australia — House of Representatives Register of Members' Interests adapter
//! (regime code `australia_register`, goal 063). SCAFFOLD ONLY — no logic yet.
//!
//! The authoritative methodology is `docs/regimes/australia_register.md`:
//! §2 discovery + the Azure-WAF browser-engine fetch seam, §3 document anatomy
//! (compound Form A statement + Notification-of-Alteration pages; 14-category
//! grid; Self/Spouse/Dependent owner bands; `value` NULL always), §4 Silver
//! `StagingRow`, §5 the two `details` contracts (`interest`,
//! `change_notification`), §6 the LLM-vision-first extraction strategy.
//!
//! Fixtures + the test-designer's independent expected outputs, the sha256
//! Bronze pins, the browser-fetch method, and the conformance conventions
//! (ULID constants, confidence literals, `entry_fields`/`category_name`
//! normalization, DD/MM/YYYY dates, owner map) live in
//! `crates/adapters/australia_register/fixtures/MANIFEST.json`.
//!
//! Leg C (rust-builder) replaces this file with the real
//! adapter/parse/normalize/details/seed modules, registers the two details
//! schema arms in `crates/pipeline/src/conformance.rs`, snapshot-commits the
//! schema files, and ships a `conformance_entry` bin.
