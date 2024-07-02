use crate::indexer::Job;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
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
