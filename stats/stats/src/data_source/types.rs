use chrono::Utc;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    /// If `None`, it will be measured at the start of update
    /// (i.e. after taking mutexes)
    pub update_time_override: Option<chrono::DateTime<Utc>>,
    /// Force full re-update
    pub force_full: bool,
}

#[derive(Clone)]
pub struct UpdateContext<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    /// Update time
    pub time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

impl<'a> From<UpdateParameters<'a>> for UpdateContext<'a> {
    fn from(value: UpdateParameters<'a>) -> Self {
        Self {
            db: value.db,
            blockscout: value.blockscout,
            time: value.update_time_override.unwrap_or_else(Utc::now),
            force_full: value.force_full,
        }
    }
}
