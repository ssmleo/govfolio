//! govfolio worker: consumers and offline bins. The alert dispatcher
//! (goal 030, design §6.3) lives in [`alerts`]; the local pipeline runner is
//! the `local` bin.

pub mod alerts;
