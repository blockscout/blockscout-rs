// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use chrono::NaiveDateTime;

use super::interval::FailedInterval;

/// Capped exponential backoff:
///   `next_attempt_at = last_attempt_at + min(base * 2^(attempts - 1), cap)`
/// Returns `true` when `interval` is due for another replay attempt.
///
/// `now` is a parameter (not read from the system clock) so the decision is
/// deterministic and testable.
pub fn is_due(
    interval: &FailedInterval,
    now: NaiveDateTime,
    base: Duration,
    cap: Duration,
) -> bool {
    let backoff_secs = capped_backoff_secs(interval.attempts, base.as_secs(), cap.as_secs());

    // `chrono::Duration::seconds` panics above `i64::MAX / 1_000` seconds
    // (~9.2e15), which a misconfigured `backoff_cap` (a raw config value,
    // not bounded by this function) can exceed even after the
    // `i64::MAX`-clamp above — `try_seconds` is the non-panicking
    // constructor (`.memory-bank/rules/error-handling.md`: no panics in
    // runtime paths). `None` means the offset cannot be represented at all,
    // which is effectively "unreasonably far in the future" for any
    // realistic `cap`; treat it as due rather than panicking or silently
    // never retrying.
    let Some(backoff) = chrono::Duration::try_seconds(backoff_secs.min(i64::MAX as u64) as i64)
    else {
        return true;
    };

    match interval.last_attempt_at.checked_add_signed(backoff) {
        Some(next_attempt_at) => now >= next_attempt_at,
        // An offset so large it cannot be represented is effectively
        // "unreasonably far in the future" for any realistic `cap`; treat it
        // as due rather than silently never retrying.
        None => true,
    }
}

/// `min(base * 2^(attempts - 1), cap)` in whole seconds, with saturating
/// arithmetic throughout. `attempts` grows without bound because holes are
/// retried forever, so `base * 2^attempts` **will** overflow eventually —
/// saturate at `cap` rather than panic or wrap.
fn capped_backoff_secs(attempts: u32, base_secs: u64, cap_secs: u64) -> u64 {
    let exponent = attempts.saturating_sub(1);
    // `checked_shl` returns `None` once the shift amount reaches the type's
    // bit width; saturate to `u64::MAX` instead of panicking.
    let multiplier = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    base_secs.saturating_mul(multiplier).min(cap_secs)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn base_ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    fn interval_with(attempts: u32, last_attempt_at: NaiveDateTime) -> FailedInterval {
        FailedInterval {
            range: super::super::interval::BlockRange { from: 1, to: 1 },
            attempts,
            reason: None,
            first_failed_at: last_attempt_at,
            last_attempt_at,
        }
    }

    #[test]
    fn capped_backoff_widens_strictly_until_the_cap() {
        let base_secs = 30;
        let cap_secs = 3600;

        let mut previous = 0;
        for attempts in 1..=7 {
            let backoff = capped_backoff_secs(attempts, base_secs, cap_secs);
            assert!(
                backoff > previous,
                "attempts={attempts} did not widen: {backoff} <= {previous}"
            );
            assert!(
                backoff < cap_secs,
                "attempts={attempts} reached the cap too early"
            );
            previous = backoff;
        }
    }

    #[test]
    fn capped_backoff_is_constant_at_the_cap_afterwards() {
        let base_secs = 30;
        let cap_secs = 3600;

        for attempts in [8, 9, 20, 1_000] {
            assert_eq!(capped_backoff_secs(attempts, base_secs, cap_secs), cap_secs);
        }
    }

    #[test]
    fn capped_backoff_does_not_overflow_at_extreme_attempts() {
        let backoff = capped_backoff_secs(u32::MAX, 30, 3600);
        assert_eq!(backoff, 3600);
    }

    #[test]
    fn is_due_is_false_before_the_backoff_elapses() {
        let last_attempt_at = base_ts();
        let interval = interval_with(3, last_attempt_at); // backoff = 30*2^2 = 120s
        let now = last_attempt_at + chrono::Duration::seconds(60);

        assert!(!is_due(
            &interval,
            now,
            Duration::from_secs(30),
            Duration::from_secs(3600)
        ));
    }

    #[test]
    fn is_due_is_true_once_the_backoff_elapses() {
        let last_attempt_at = base_ts();
        let interval = interval_with(3, last_attempt_at); // backoff = 120s
        let now = last_attempt_at + chrono::Duration::seconds(121);

        assert!(is_due(
            &interval,
            now,
            Duration::from_secs(30),
            Duration::from_secs(3600)
        ));
    }

    #[test]
    fn is_due_still_fires_after_a_very_large_attempts_count_once_the_cap_elapses() {
        let last_attempt_at = base_ts();
        let interval = interval_with(u32::MAX, last_attempt_at);
        let now = last_attempt_at + chrono::Duration::seconds(3601);

        assert!(is_due(
            &interval,
            now,
            Duration::from_secs(30),
            Duration::from_secs(3600)
        ));
    }

    /// `chrono::Duration::seconds` panics above `i64::MAX / 1_000`
    /// (~9.2e15) seconds; `backoff_cap` comes straight from config, so a
    /// misconfigured value that large (or `attempts` large enough that
    /// `capped_backoff_secs` saturates at it) must not panic — `is_due`
    /// must still return a plain `bool`.
    #[test]
    fn is_due_does_not_panic_on_a_backoff_beyond_chrono_duration_bounds() {
        let last_attempt_at = base_ts();
        let base = Duration::from_secs(30);
        let huge_cap = Duration::from_secs(u64::MAX);

        // Guard the *inputs*, not just the outcome. A huge `cap` alone never
        // reaches the unrepresentable-offset branch, because `capped_backoff`
        // takes the `min` of the two — so a test written with a small
        // `attempts` passes identically against the panicking
        // `Duration::seconds`, guarding nothing. `attempts = 64` saturates the
        // shift and takes the backoff past chrono's `i64::MAX / 1_000` ceiling
        // regardless of `base`.
        let attempts = 64;
        assert!(
            capped_backoff_secs(attempts, base.as_secs(), huge_cap.as_secs())
                > (i64::MAX / 1_000) as u64,
            "inputs no longer reach the branch this test exists to cover"
        );

        let interval = interval_with(attempts, last_attempt_at);
        let now = last_attempt_at + chrono::Duration::seconds(1);

        // An offset that cannot be represented is treated as due — never a
        // panic, and never a hole that is silently retried never again.
        assert!(is_due(&interval, now, base, huge_cap));
    }
}
