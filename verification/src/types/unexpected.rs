//! Error utils

use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Error indicating an expected value was not found.
pub struct Mismatch<T> {
    /// Value expected.
    pub expected: T,
    /// Value found.
    pub found: Option<T>,
}

impl<T> Mismatch<T> {
    /// Creates an error with both `expected` and `found` values.
    pub fn new(expected: T, found: T) -> Self {
        Self {
            expected,
            found: Some(found),
        }
    }

    /// Creates an error when `found` value is missing.
    pub fn expected(expected: T) -> Self {
        Self {
            expected,
            found: None,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Mismatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("Expected {}", self.expected))?;
        match &self.found {
            Some(found) => f.write_fmt(format_args!(", found {}", found)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::types::Mismatch;

    #[test]
    fn test_display_mismatch_with_found() {
        // given
        let expected = 1;
        let found = 2;
        let mismatch = Mismatch::new(expected, found);

        // when
        let actual = format!("{}", mismatch);

        // then
        assert_eq!(format!("Expected {}, found {}", expected, found), actual);
    }

    #[test]
    fn test_display_mismatch_without_found() {
        // given
        let expected = 1;
        let mismatch = Mismatch::expected(expected);

        // when
        let actual = format!("{}", mismatch);

        // then
        assert_eq!(format!("Expected {}", expected), actual);
    }
}
