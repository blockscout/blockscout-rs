use super::TimespanValue;

/// Marked as precise or approximate
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtendedTimespanValue<T, V> {
    pub timespan: T,
    pub value: V,
    pub is_approximate: bool,
}

impl<T, V> ExtendedTimespanValue<T, V> {
    pub fn from_date_value(dv: TimespanValue<T, V>, is_approximate: bool) -> Self {
        Self {
            timespan: dv.timespan,
            value: dv.value,
            is_approximate,
        }
    }
}

impl<T, V> From<ExtendedTimespanValue<T, V>> for TimespanValue<T, V> {
    fn from(dv: ExtendedTimespanValue<T, V>) -> Self {
        Self {
            timespan: dv.timespan,
            value: dv.value,
        }
    }
}
