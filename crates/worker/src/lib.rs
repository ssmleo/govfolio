//! govfolio worker: consumers and offline bins. The alert dispatcher
//! (goal 030, design §6.3) lives in [`alerts`]; usage -> Stripe metering
//! (goal 050) in [`billing`] over the [`stripe`] seam; the bulk export in
//! [`snapshot`]; the local pipeline runner is the `local` bin.

pub mod alerts;
pub mod billing;
pub mod snapshot;
pub mod stripe;
