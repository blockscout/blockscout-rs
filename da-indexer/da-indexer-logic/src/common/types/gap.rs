use std::fmt;

use sea_orm::FromQueryResult;

#[derive(FromQueryResult, Clone)]
pub struct Gap {
    pub start: i64,
    pub end: i64,
}

impl Gap {
    pub fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }
}

impl fmt::Display for Gap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

impl fmt::Debug for Gap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}
