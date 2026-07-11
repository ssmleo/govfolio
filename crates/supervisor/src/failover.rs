//! Bounded provider failover policy for the fenced logical orchestrator lane.

use crate::model::{Provider, ResultClass};

/// The only cross-provider action allowed for one work unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailoverAction {
    /// Do not spend an alternate-provider turn.
    Stay,
    /// Start one fresh alternate-provider recovery under the current lane fence.
    FreshAlternate,
    /// The bounded recovery was consumed or failed; fence the lane for reconciliation.
    FenceRecovery,
}

/// Tracks the provider-neutral budget for one work unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailoverBudget {
    initial_provider: Provider,
    alternate_used: bool,
}

impl FailoverBudget {
    #[must_use]
    pub const fn new(initial_provider: Provider) -> Self {
        Self {
            initial_provider,
            alternate_used: false,
        }
    }

    #[must_use]
    pub const fn initial_provider(self) -> Provider {
        self.initial_provider
    }

    /// Decides whether the failed result may spend the single alternate turn.
    ///
    /// Quota, authentication, policy, operator-stop, and ambiguous outcomes are
    /// deliberately excluded. They are handled by circuits or reconciliation,
    /// never by an immediate extra model launch.
    #[must_use]
    pub fn decide(
        self,
        result: ResultClass,
        alternate_provider: Provider,
        alternate_proven: bool,
    ) -> FailoverAction {
        if self.alternate_used {
            return FailoverAction::FenceRecovery;
        }
        if alternate_provider == self.initial_provider || !alternate_proven {
            return FailoverAction::FenceRecovery;
        }
        match result {
            ResultClass::TransientTransport | ResultClass::ProviderUnavailable => {
                FailoverAction::FreshAlternate
            }
            ResultClass::SpawnFailed
            | ResultClass::SessionInvalid
            | ResultClass::PostconditionFailed => FailoverAction::FenceRecovery,
            ResultClass::Completed
            | ResultClass::OperatorStop
            | ResultClass::RateLimited
            | ResultClass::QuotaExhausted
            | ResultClass::Auth
            | ResultClass::RunnerConfig
            | ResultClass::Policy
            | ResultClass::Ambiguous => FailoverAction::Stay,
        }
    }

    #[must_use]
    pub const fn consume_alternate(mut self) -> Self {
        self.alternate_used = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failover_allows_one_fresh_cross_provider_recovery() {
        let budget = FailoverBudget::new(Provider::Codex);
        assert_eq!(
            budget.decide(ResultClass::TransientTransport, Provider::Claude, true,),
            FailoverAction::FreshAlternate
        );
        assert_eq!(
            budget.consume_alternate().decide(
                ResultClass::TransientTransport,
                Provider::Claude,
                true,
            ),
            FailoverAction::FenceRecovery
        );
    }

    #[test]
    fn failover_requires_a_proven_different_provider() {
        let budget = FailoverBudget::new(Provider::Codex);
        assert_eq!(
            budget.decide(ResultClass::ProviderUnavailable, Provider::Claude, false,),
            FailoverAction::FenceRecovery
        );
        assert_eq!(
            budget.decide(ResultClass::ProviderUnavailable, Provider::Codex, true),
            FailoverAction::FenceRecovery
        );
    }

    #[test]
    fn failover_fences_dirty_or_unreconciled_state() {
        let budget = FailoverBudget::new(Provider::Codex);
        for result in [
            ResultClass::SpawnFailed,
            ResultClass::SessionInvalid,
            ResultClass::PostconditionFailed,
        ] {
            assert_eq!(
                budget.decide(result, Provider::Claude, true),
                FailoverAction::FenceRecovery
            );
        }
    }

    #[test]
    fn failover_never_spends_on_closed_classes() {
        let budget = FailoverBudget::new(Provider::Codex);
        for result in [
            ResultClass::QuotaExhausted,
            ResultClass::RateLimited,
            ResultClass::Auth,
            ResultClass::RunnerConfig,
            ResultClass::Policy,
            ResultClass::OperatorStop,
            ResultClass::Ambiguous,
        ] {
            assert_eq!(
                budget.decide(result, Provider::Claude, true),
                FailoverAction::Stay,
                "unexpected alternate spend for {result}"
            );
        }
    }
}
