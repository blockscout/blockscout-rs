use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Default)]
#[serde(crate = "fang::serde")]
pub struct CheckBalanceTask {}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for CheckBalanceTask {
    async fn run(&self, _client: &mut dyn AsyncQueueable) -> Result<(), FangError> {
        tracing::info!("checking balance");
        let _db = super::global::get_db_connection();
        Ok(())
    }

    fn uniq(&self) -> bool {
        true
    }

    fn cron(&self) -> Option<Scheduled> {
        Some(Scheduled::CronPattern("0 * * * * *".to_string()))
    }
}
