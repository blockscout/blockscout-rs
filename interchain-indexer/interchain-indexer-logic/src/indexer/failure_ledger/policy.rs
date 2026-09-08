// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use chrono::NaiveDateTime;

/// Capped exponential backoff:
///   `next_attempt_at = last_attempt_at + min(base * 2^(attempts - 1), cap)`
///
/// The instant is returned rather than a due/not-due boolean because the retry
/// scheduler stores it: a session's due time is decided once, when it is
/// bootstrapped or when a sweep completes, and is deliberately not recomputed
/// from the ledger row on every tick.
///
/// `None` deliberately means immediately due. It preserves the failure-open
/// behaviour for offsets chrono cannot represent, rather than panicking or
/// parking a durable failure forever. An attempts value of zero uses the base
/// delay, which is required after a partially successful sweep resets the
/// scheduler-local counter.
pub(crate) fn next_attempt_at(
    last_attempt_at: NaiveDateTime,
    attempts: u32,
    base: Duration,
    cap: Duration,
) -> Option<NaiveDateTime> {
    let backoff_secs = capped_backoff_secs(attempts, base.as_secs(), cap.as_secs());

    // `chrono::Duration::seconds` panics above `i64::MAX / 1_000` seconds
    // (~9.2e15), which a misconfigured `backoff_cap` (a raw config value,
    // not bounded by this function) can exceed even after the
    // `i64::MAX`-clamp above — `try_seconds` is the non-panicking
    // constructor (`.memory-bank/rules/error-handling.md`: no panics in
    // runtime paths). `None` means the offset cannot be represented at all,
    // which is effectively "unreasonably far in the future" for any
    // realistic `cap`; treat it as due rather than panicking or silently
    // never retrying.
    let backoff = chrono::Duration::try_seconds(backoff_secs.min(i64::MAX as u64) as i64)?;

    // An offset so large it cannot be represented is treated as immediately
    // due by the scheduler rather than silently never retried.
    last_attempt_at.checked_add_signed(backoff)
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
    fn next_attempt_at_offsets_the_last_attempt_by_the_capped_backoff() {
        let last_attempt_at = base_ts();
        let base = Duration::from_secs(30);
        let cap = Duration::from_secs(3600);

        // attempts = 3 => 30 * 2^2 = 120s.
        let due =
            next_attempt_at(last_attempt_at, 3, base, cap).expect("a 120s offset is representable");

        assert_eq!(due, last_attempt_at + chrono::Duration::seconds(120));
        assert!(last_attempt_at + chrono::Duration::seconds(60) < due);
        assert!(last_attempt_at + chrono::Duration::seconds(121) > due);
    }

    /// Zero is a real input, not a guarded one: the scheduler passes it after a
    /// sweep made progress, and it must mean the base delay rather than "now".
    #[test]
    fn next_attempt_at_uses_the_base_delay_for_zero_attempts() {
        let last_attempt_at = base_ts();

        let due = next_attempt_at(
            last_attempt_at,
            0,
            Duration::from_secs(30),
            Duration::from_secs(3600),
        )
        .expect("a 30s offset is representable");

        assert_eq!(due, last_attempt_at + chrono::Duration::seconds(30));
    }

    #[test]
    fn next_attempt_at_still_lands_at_the_cap_for_a_huge_attempt_count() {
        let last_attempt_at = base_ts();

        let due = next_attempt_at(
            last_attempt_at,
            u32::MAX,
            Duration::from_secs(30),
            Duration::from_secs(3600),
        )
        .expect("the cap is representable");

        assert_eq!(due, last_attempt_at + chrono::Duration::seconds(3600));
    }

    /// `chrono::Duration::seconds` panics above `i64::MAX / 1_000`
    /// (~9.2e15) seconds; `backoff_cap` comes straight from config, so a
    /// misconfigured value that large (or `attempts` large enough that
    /// `capped_backoff_secs` saturates at it) must not panic — it must return
    /// `None`, which every caller reads as immediately due.
    #[test]
    fn next_attempt_at_is_none_on_a_backoff_beyond_chrono_duration_bounds() {
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

        // Never a panic, and never a hole that is silently retried never again.
        assert_eq!(next_attempt_at(base_ts(), attempts, base, huge_cap), None);
    }

    /// The other `None` branch: a representable offset added to a timestamp
    /// that cannot absorb it.
    #[test]
    fn next_attempt_at_is_none_when_the_timestamp_cannot_absorb_the_offset() {
        assert_eq!(
            next_attempt_at(
                NaiveDateTime::MAX,
                3,
                Duration::from_secs(30),
                Duration::from_secs(3600)
            ),
            None
        );
    }
}
