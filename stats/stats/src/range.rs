use std::ops::{Bound, Range, RangeBounds, RangeInclusive};

use chrono::{DateTime, Utc};

use crate::{
    data_source::{kinds::remote_db::RemoteQueryBehaviour, UpdateContext},
    types::{Timespan, TimespanDuration},
    UpdateError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniversalRange<Idx> {
    /// Always inclusive, if present
    pub start: Option<Idx>,
    pub end: Bound<Idx>,
}

impl<Idx> UniversalRange<Idx> {
    pub fn full() -> Self {
        Self {
            start: None,
            end: Bound::Unbounded,
        }
    }

    pub fn with_replaced_unbounded(self, replacement_source: Range<Idx>) -> Self {
        let start = match self.start {
            Some(s) => Some(s),
            None => Some(replacement_source.start),
        };
        let end = match self.end {
            Bound::Unbounded => Bound::Excluded(replacement_source.end),
            _ => self.end,
        };
        Self { start, end }
    }

    pub fn map<T>(self, f: impl Fn(Idx) -> T) -> UniversalRange<T> {
        UniversalRange {
            start: self.start.map(&f),
            end: self.end.map(f),
        }
    }
}

impl<Idx> From<Range<Idx>> for UniversalRange<Idx> {
    fn from(value: Range<Idx>) -> Self {
        Self {
            start: Some(value.start),
            end: Bound::Excluded(value.end),
        }
    }
}

impl<Idx> From<Range<Option<Idx>>> for UniversalRange<Idx> {
    fn from(value: Range<Option<Idx>>) -> Self {
        Self {
            start: value.start,
            end: match value.end {
                Some(e) => Bound::Excluded(e),
                None => Bound::Unbounded,
            },
        }
    }
}

impl<Idx> From<RangeInclusive<Idx>> for UniversalRange<Idx> {
    fn from(value: RangeInclusive<Idx>) -> Self {
        let (start, end) = value.into_inner();
        Self {
            start: Some(start),
            end: Bound::Included(end),
        }
    }
}

impl<Idx> From<RangeInclusive<Option<Idx>>> for UniversalRange<Idx> {
    fn from(value: RangeInclusive<Option<Idx>>) -> Self {
        let (start, end) = value.into_inner();
        Self {
            start: start,
            end: match end {
                Some(e) => Bound::Included(e),
                None => Bound::Unbounded,
            },
        }
    }
}

impl<Idx: PartialEq<Idx>> UniversalRange<Idx> {
    pub fn is_unbounded(&self) -> bool {
        self.start.is_none() || self.end == Bound::Unbounded
    }
}

impl<Idx: Incrementable> UniversalRange<Idx> {
    /// None only if no backup is provided
    fn into_exclusive_inner(self, backup_bounds: Option<Range<Idx>>) -> Option<Range<Idx>> {
        let (backup_start, backup_end) = backup_bounds.map(|b| (b.start, b.end)).unzip();
        // todo: test that none backup and some start works as expected (+ end)
        let start = self.start.or(backup_start)?;
        match self.end {
            Bound::Included(end) => Some(inclusive_range_to_exclusive(start..=end)),
            Bound::Excluded(end) => Some(start..end),
            Bound::Unbounded => {
                let end = backup_end?;
                Some(start..end)
            }
        }
    }

    /// `None` if any of the ends is unbounded
    pub fn try_into_exclusive(self) -> Option<Range<Idx>> {
        self.into_exclusive_inner(None)
    }

    /// Take bounds from `backup_bounds` if any of them is unbounded.
    pub fn into_exclusive_with_backup(self, backup_bounds: Range<Idx>) -> Range<Idx> {
        self.into_exclusive_inner(Some(backup_bounds))
            .expect("`backup_bounds` is not None")
    }
}

impl<Idx: Incrementable + Decrementable + Ord> UniversalRange<Idx> {
    fn into_inclusive_inner(
        self,
        backup_bounds: Option<RangeInclusive<Idx>>,
    ) -> Option<RangeInclusive<Idx>> {
        let (backup_start, backup_end) = backup_bounds.map(|b| b.into_inner()).unzip();
        let start = self.start.or(backup_start)?;
        match self.end {
            Bound::Included(end) => Some(start..=end),
            Bound::Excluded(end) => Some(exclusive_range_to_inclusive(start..end)),
            Bound::Unbounded => {
                let end = backup_end?;
                Some(start..=end)
            }
        }
    }

    /// `None` if any of the ends is unbounded
    pub fn try_into_inclusive(self) -> Option<RangeInclusive<Idx>> {
        self.into_inclusive_inner(None)
    }

    /// Take bounds from `backup_bounds` if any of them is unbounded.
    pub fn into_inclusive_with_backup(
        self,
        backup_bounds: RangeInclusive<Idx>,
    ) -> RangeInclusive<Idx> {
        self.into_inclusive_inner(Some(backup_bounds))
            .expect("`backup_bounds` is not None")
    }

    pub fn into_inclusive_pair(self) -> (Option<Idx>, Option<Idx>) {
        match (self.start, self.end) {
            (start, Bound::Unbounded) => (start, None),
            (start, Bound::Included(e)) => (start, Some(e)),
            (None, Bound::Excluded(e)) => (None, Some(e.saturating_dec())),
            (Some(s), Bound::Excluded(e)) => {
                Some(exclusive_range_to_inclusive(s..e).into_inner()).unzip()
            }
        }
    }
}

impl<Idx: PartialOrd<Idx>> RangeBounds<Idx> for UniversalRange<Idx> {
    fn start_bound(&self) -> Bound<&Idx> {
        match &self.start {
            Some(s) => Bound::Included(s),
            None => Bound::Unbounded,
        }
    }

    fn end_bound(&self) -> Bound<&Idx> {
        self.end.as_ref()
    }
}

pub fn exclusive_range_to_inclusive<Idx: Incrementable + Decrementable + Ord>(
    r: Range<Idx>,
) -> RangeInclusive<Idx> {
    let mut start = r.start;
    let end = match r.end.checked_dec() {
        Some(new_end) => new_end,
        None => {
            // current end is the minimum value and is excluded, thus
            // `self` range is empty
            // so we need to produce an empty range as well
            if start.checked_dec().is_none() {
                // will not produce an empty range iff there is only
                // one value of `Idx` type, which will not be encountered
                // in practice
                start = start.saturating_inc();
            }
            r.end
        }
    };
    start..=end
}

pub fn inclusive_range_to_exclusive<Idx: Incrementable>(r: RangeInclusive<Idx>) -> Range<Idx> {
    let (start, end) = r.into_inner();
    // impossible to include max value in exclusive range,
    // so we leave it excluded in such case
    start..end.saturating_inc()
}

pub trait Incrementable {
    fn saturating_inc(&self) -> Self;
    fn checked_inc(&self) -> Option<Self>
    where
        Self: Sized;
}

impl<T: Timespan> Incrementable for T {
    fn saturating_inc(&self) -> Self {
        self.saturating_next_timespan()
    }

    fn checked_inc(&self) -> Option<Self>
    where
        Self: Sized,
    {
        self.checked_add(TimespanDuration::from_timespan_repeats(1))
    }
}

impl Incrementable for DateTime<Utc> {
    fn saturating_inc(&self) -> Self {
        self.checked_inc().unwrap_or(DateTime::<Utc>::MAX_UTC)
    }

    fn checked_inc(&self) -> Option<Self> {
        self.checked_add_signed(chrono::Duration::nanoseconds(1))
    }
}

// for tests
impl Incrementable for i32 {
    fn saturating_inc(&self) -> Self {
        self.saturating_add(1)
    }

    fn checked_inc(&self) -> Option<Self> {
        self.checked_add(1)
    }
}

pub trait Decrementable {
    fn saturating_dec(&self) -> Self;
    fn checked_dec(&self) -> Option<Self>
    where
        Self: Sized;
}

impl<T: Timespan> Decrementable for T {
    fn saturating_dec(&self) -> Self {
        self.saturating_previous_timespan()
    }

    fn checked_dec(&self) -> Option<Self>
    where
        Self: Sized,
    {
        self.checked_sub(TimespanDuration::from_timespan_repeats(1))
    }
}

impl Decrementable for DateTime<Utc> {
    fn saturating_dec(&self) -> Self {
        self.checked_dec().unwrap_or(DateTime::<Utc>::MIN_UTC)
    }

    fn checked_dec(&self) -> Option<Self> {
        self.checked_sub_signed(chrono::Duration::nanoseconds(1))
    }
}

impl Decrementable for i32 {
    fn saturating_dec(&self) -> Self {
        self.saturating_sub(1)
    }

    fn checked_dec(&self) -> Option<Self> {
        self.checked_sub(1)
    }
}

// todo (future releases/features): implement one-sided range support
// for db statements (in `filter` part)
/// DB statements have a regular range to keep their implementation simpler
/// but data sources have flexible bound setting. This function converts
/// from one to another while ensuring that non-range queries stay the same.
///
/// At the same time, it adds support for one-side unbounded ranges by
/// quering the provided all range source in such case.
pub async fn data_source_query_range_to_db_statement_range<AllRangeSource>(
    cx: &UpdateContext<'_>,
    data_source_range: UniversalRange<DateTime<Utc>>,
) -> Result<Option<Range<DateTime<Utc>>>, UpdateError>
where
    AllRangeSource: RemoteQueryBehaviour<Output = Range<DateTime<Utc>>>,
{
    let range = if let Some(r) = data_source_range.clone().try_into_exclusive() {
        Some(r)
    } else if data_source_range.start.is_none() && data_source_range.end == Bound::Unbounded {
        None
    } else {
        let whole_range = AllRangeSource::query_data(cx, UniversalRange::full()).await?;
        Some(data_source_range.into_exclusive_with_backup(whole_range))
    };
    Ok(range)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tests::point_construction::*, types::timespans::Week};
    use chrono::{NaiveDate, NaiveDateTime};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_with_replaced_unbounded() {
        let range = UniversalRange {
            start: None,
            end: Bound::Unbounded,
        };
        let replacement = dt("2023-01-01T00:00:00")..dt("2023-01-02T00:00:00");
        let result = range.with_replaced_unbounded(replacement);
        assert_eq!(result.start, Some(dt("2023-01-01T00:00:00")));
        assert_eq!(result.end, Bound::Excluded(dt("2023-01-02T00:00:00")));

        let partial_range = UniversalRange {
            start: Some(dt("2023-01-01T12:00:00")),
            end: Bound::Unbounded,
        };
        let result = partial_range
            .with_replaced_unbounded(dt("2023-01-01T00:00:00")..dt("2023-01-02T00:00:00"));
        assert_eq!(result.start, Some(dt("2023-01-01T12:00:00")));
        assert_eq!(result.end, Bound::Excluded(dt("2023-01-02T00:00:00")));
    }

    #[test]
    fn test_conversion_functions() {
        // Test with weeks
        let exclusive = d("2023-01-01")..d("2023-01-15");
        let inclusive = exclusive_range_to_inclusive(exclusive);
        assert_eq!(inclusive, d("2023-01-01")..=d("2023-01-14"));

        let inclusive = d("2023-01-01")..=d("2023-01-14");
        let exclusive = inclusive_range_to_exclusive(inclusive);
        assert_eq!(exclusive, d("2023-01-01")..d("2023-01-15"));

        // Test with months
        let exclusive = month_of("2023-01-01")..month_of("2023-03-01");
        let inclusive = exclusive_range_to_inclusive(exclusive);
        assert_eq!(inclusive, month_of("2023-01-01")..=month_of("2023-02-01"));
    }

    #[test]
    fn test_into_exclusive_conversions() {
        let range: UniversalRange<NaiveDate> = (d("2023-01-01")..d("2023-01-15")).into();
        assert_eq!(
            range.clone().try_into_exclusive(),
            Some(d("2023-01-01")..d("2023-01-15"))
        );

        let inclusive: UniversalRange<NaiveDate> = (d("2023-01-01")..=d("2023-01-14")).into();
        assert_eq!(
            inclusive.clone().try_into_exclusive(),
            Some(d("2023-01-01")..d("2023-01-15"))
        );

        let unbounded = UniversalRange::full();
        assert_eq!(unbounded.clone().try_into_exclusive(), None);

        let with_backup = unbounded.into_exclusive_with_backup(d("2023-01-01")..d("2023-01-15"));
        assert_eq!(with_backup, d("2023-01-01")..d("2023-01-15"));
    }

    #[test]
    fn test_into_inclusive_conversions() {
        let range: UniversalRange<NaiveDate> = (d("2023-01-01")..=d("2023-01-15")).into();
        assert_eq!(
            range.clone().try_into_inclusive(),
            Some(d("2023-01-01")..=d("2023-01-15"))
        );

        let exclusive: UniversalRange<NaiveDate> = (d("2023-01-01")..d("2023-01-15")).into();
        assert_eq!(
            exclusive.clone().try_into_inclusive(),
            Some(d("2023-01-01")..=d("2023-01-14"))
        );

        let unbounded = UniversalRange::full();
        assert_eq!(unbounded.clone().try_into_inclusive(), None);

        let with_backup = unbounded.into_inclusive_with_backup(d("2023-01-01")..=d("2023-01-08"));
        assert_eq!(with_backup, d("2023-01-01")..=d("2023-01-08"));
    }

    #[test]
    fn test_into_inclusive_pair() {
        // Basic inclusive range
        let inclusive: UniversalRange<i32> = (1..=5).into();
        assert_eq!(inclusive.into_inclusive_pair(), (Some(1), Some(5)));

        // Basic exclusive range
        let exclusive: UniversalRange<i32> = (1..6).into();
        assert_eq!(exclusive.into_inclusive_pair(), (Some(1), Some(5)));

        // Unbounded range
        let unbounded = UniversalRange::<i32>::full();
        assert_eq!(unbounded.into_inclusive_pair(), (None, None));

        // Start-only range
        let start_only = UniversalRange {
            start: Some(1),
            end: Bound::Unbounded,
        };
        assert_eq!(start_only.into_inclusive_pair(), (Some(1), None));

        // End-only range (exclusive)
        let end_only = UniversalRange {
            start: None,
            end: Bound::Excluded(5),
        };
        assert_eq!(end_only.into_inclusive_pair(), (None, Some(4)));

        // End-only range (inclusive)
        let end_only_inclusive = UniversalRange {
            start: None,
            end: Bound::Included(5),
        };
        assert_eq!(end_only_inclusive.into_inclusive_pair(), (None, Some(5)));
    }

    #[test]
    fn test_exclusive_to_inclusive_conversion() {
        // Normal case
        assert_eq!(exclusive_range_to_inclusive(1..5), 1..=4);

        // Single-element range
        assert_eq!(exclusive_range_to_inclusive(1..2), 1..=1);

        // Empty range at start
        let empty_start = exclusive_range_to_inclusive(0..0);
        assert!(empty_start.is_empty());

        // Empty range at end of i32
        let empty_end = exclusive_range_to_inclusive(i32::MAX..i32::MAX);
        assert!(empty_end.is_empty());

        // Range ending at i32::MAX
        assert_eq!(
            exclusive_range_to_inclusive(0..i32::MAX),
            0..=(i32::MAX - 1)
        );

        // Minimal non-empty range
        assert_eq!(exclusive_range_to_inclusive(5..6), 5..=5);
    }

    #[test]
    fn test_inclusive_to_exclusive_conversion() {
        // Normal case
        assert_eq!(inclusive_range_to_exclusive(1..=4), 1..5);

        // Single element range
        assert_eq!(inclusive_range_to_exclusive(1..=1), 1..2);

        // can't represent (i32::MAX..=i32::MAX) as an exclusive range
        // so no test for that

        // Range ending at i32::MAX
        assert_eq!(
            inclusive_range_to_exclusive(0..=i32::MAX),
            0..i32::MAX // Note: this saturates at MAX instead of overflowing
        );

        // Range ending one before MAX
        assert_eq!(
            inclusive_range_to_exclusive(0..=(i32::MAX - 1)),
            0..i32::MAX
        );

        // Test with minimum values
        assert_eq!(
            inclusive_range_to_exclusive(i32::MIN..=i32::MIN),
            i32::MIN..(i32::MIN + 1)
        );
    }
}
