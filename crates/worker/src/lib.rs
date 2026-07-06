//! govfolio worker: consumers and offline bins. The alert dispatcher
//! (goal 030, design §6.3) lives in [`alerts`]; usage -> Stripe metering
//! (goal 050) in [`billing`] over the [`stripe`] seam; the bulk export in
//! [`snapshot`]; continuous drift defense (goal 017, design §5.6/§5.8) in
//! [`sentinel`]; the monthly sampling audit + precision report (goal 070,
//! design §7.4) in [`sampler`]; the local pipeline runner is the `local` bin.

pub mod alerts;
pub mod billing;
pub mod sampler;
pub mod sentinel;
pub mod snapshot;
pub mod stripe;
