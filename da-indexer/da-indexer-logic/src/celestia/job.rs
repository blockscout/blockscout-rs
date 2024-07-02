use std::fmt;

use crate::indexer::Job;

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct CelestiaJob {
    pub height: u64,
}

impl From<Job> for CelestiaJob {
    fn from(val: Job) -> Self {
        match val {
            Job::Celestia(job) => job,
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for CelestiaJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "height = {}", self.height)
    }
}
