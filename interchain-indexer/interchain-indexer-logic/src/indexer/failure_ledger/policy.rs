// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use chrono::NaiveDateTime;

/// Capped exponential backoff:
///   `next_attempt_at = last_attempt_at + min(base * (9/8)^(attempts - 1), cap)`
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

/// `min(base * (9/8)^(attempts - 1), cap)` in whole seconds. `9/8` (12.5%
/// growth per attempt) replaces a `2^n` doubling because doubling saturates
/// `backoff_cap` far too fast to be useful: with the default `base=30s` /
/// `cap=3600s`, `2^7 * 30s` already exceeds the cap, so attempt 8 is
/// wholly indistinguishable from attempt 800 — every transient
/// provider/RPC hiccup that outlives a few minutes gets parked at the full
/// hour, right when it is most likely to still be recoverable. `9/8`
/// keeps widening for ~40 attempts before flattening out at the same cap,
/// trading a slightly slower retreat to the ceiling for far more retry
/// density during the window realistic outages actually clear in.
///
/// `attempts` grows without bound (holes are retried forever), so the
/// exponent is clamped before the cast to `i32`: `(9/8)^exponent` is
/// already `f64::INFINITY` well before `exponent = 10_000` (it overflows
/// around exponent ~6026), so the clamp loses no precision and keeps the
/// cast safe for any `u32` input. `f64 -> u64` casts saturate in Rust (no
/// panic, no UB), so an infinite or out-of-range backoff becomes
/// `u64::MAX` here, which `.min(cap_secs)` then correctly clamps down to
/// `cap_secs`.
fn capped_backoff_secs(attempts: u32, base_secs: u64, cap_secs: u64) -> u64 {
    let exponent = attempts.saturating_sub(1).min(10_000) as i32;
    let backoff = base_secs as f64 * (9.0_f64 / 8.0).powi(exponent);
    backoff.min(cap_secs as f64) as u64
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
    fn capped_backoff_widens_for_far_more_than_the_old_doubling_horizon() {
        let base_secs = 30;
        let cap_secs = 3600;

        // A `2^n` doubling saturated by attempt 8; `9/8` growth must still be
        // strictly widening decades of attempts past that, so a transient
        // failure that clears within a few hours keeps seeing progressively
        // longer (but not yet maxed-out) gaps instead of jumping straight to
        // the 1-hour cap.
        let mut previous = 0;
        for attempts in 1..=30 {
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
    fn capped_backoff_is_constant_at_the_cap_for_large_attempt_counts() {
        let base_secs = 30;
        let cap_secs = 3600;

        for attempts in [60, 100, 1_000, u32::MAX] {
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

        // attempts = 3 => 30 * (9/8)^2 = 30 * 1.265625 = 37.96875s, truncated
        // to 37s by the `f64 -> u64` cast.
        let due =
            next_attempt_at(last_attempt_at, 3, base, cap).expect("a 37s offset is representable");

        assert_eq!(due, last_attempt_at + chrono::Duration::seconds(37));
        assert!(last_attempt_at + chrono::Duration::seconds(36) < due);
        assert!(last_attempt_at + chrono::Duration::seconds(38) > due);
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
        // `Duration::seconds`, guarding nothing. `9/8` growth is far slower
        // than the old `2^n` doubling (which crossed this ceiling by
        // `attempts = 64`), so reaching it now needs `attempts` in the
        // hundreds regardless of `base`.
        let attempts = 300;
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
