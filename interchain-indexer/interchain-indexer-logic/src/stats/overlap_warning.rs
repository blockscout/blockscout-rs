// SPDX-License-Identifier: LicenseRef-Blockscout

//! Pure warning-transition policy for the `stats_chains` bridge-sum overlap
//! signal, factored out of the recalculation worker so it is unit-testable
//! without Prometheus — `.memory-bank/rules/testing.md` forbids asserting
//! process-wide metric deltas in a test.

/// What changed between the previous and current recomputation's overlap
/// state (`previous` is `None` before the first successful recomputation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapTransition {
    /// `None`/`Some(false)` -> `true`: warn once, with totals and samples.
    Appeared,
    /// `true` -> `true`: update gauges/success log only, no repeated warning.
    Persisted,
    /// `true` -> `false`: one informational recovery log.
    Recovered,
    /// `false` -> `false`, or unchanged otherwise: nothing to say.
    Quiet,
}

/// Decides the transition for one successful recomputation. A failed
/// recomputation must not call this at all — the caller keeps the previous
/// `Option<bool>` state untouched, exactly as it keeps the previous tables and
/// gauges (see `recompute_stats_chains`'s rollback-on-error guarantee).
pub fn overlap_transition(previous: Option<bool>, current: bool) -> OverlapTransition {
    match (previous, current) {
        (Some(true), true) => OverlapTransition::Persisted,
        (Some(true), false) => OverlapTransition::Recovered,
        (_, true) => OverlapTransition::Appeared,
        (_, false) => OverlapTransition::Quiet,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlap_transition_initial_positive_is_appeared() {
        assert_eq!(overlap_transition(None, true), OverlapTransition::Appeared);
    }

    #[test]
    fn test_overlap_transition_false_to_true_is_appeared() {
        assert_eq!(
            overlap_transition(Some(false), true),
            OverlapTransition::Appeared
        );
    }

    #[test]
    fn test_overlap_transition_true_to_true_is_persisted() {
        assert_eq!(
            overlap_transition(Some(true), true),
            OverlapTransition::Persisted
        );
    }

    #[test]
    fn test_overlap_transition_true_to_false_is_recovered() {
        assert_eq!(
            overlap_transition(Some(true), false),
            OverlapTransition::Recovered
        );
    }

    #[test]
    fn test_overlap_transition_initial_negative_is_quiet() {
        assert_eq!(overlap_transition(None, false), OverlapTransition::Quiet);
    }

    #[test]
    fn test_overlap_transition_false_to_false_is_quiet() {
        assert_eq!(
            overlap_transition(Some(false), false),
            OverlapTransition::Quiet
        );
    }

    /// A failed recomputation must never call this helper — the worker keeps
    /// the previous `Option<bool>` untouched instead. There is nothing to
    /// assert on the function itself for that case; this test documents the
    /// contract so a future change does not "helpfully" add a failure input.
    #[test]
    fn test_overlap_transition_contract_failed_refresh_is_not_a_call_site() {
        // A failed run means: do not call `overlap_transition` at all, and do
        // not update `previous`. Simulate two failed attempts after an
        // `Appeared` transition by simply not calling the function, then
        // confirm the next successful call still sees the old `previous`.
        let previous = Some(true);
        // ... two failed recomputations elapse; `previous` is untouched ...
        assert_eq!(
            overlap_transition(previous, true),
            OverlapTransition::Persisted
        );
    }
}
