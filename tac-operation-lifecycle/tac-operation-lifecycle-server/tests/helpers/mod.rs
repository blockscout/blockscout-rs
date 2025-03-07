use blockscout_service_launcher::{
    test_database::TestDbGuard,
    test_server
};
use reqwest::Url;
use tac_operation_lifecycle_server::Settings;

pub async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}
pub async fn init_tac_operation_lifecycle_server<F>(
    db_url: String,
    settings_setup: F
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default(
            db_url
            );
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| tac_operation_lifecycle_server::run(settings), &base).await;
    base
}


#[cfg(test)]
mod tests {
    use std::time;
    use rand::Rng;

    use tac_operation_lifecycle_logic::{settings::IndexerSettings, Indexer};
    use tac_operation_lifecycle_entity::interval;
    use migration::sea_orm::EntityTrait;
    use super::*;
    
    #[tokio::test]
    async fn test_save_intervals() {
        let db = init_db("test_save_intervals", "test").await;
        let catchup_interval = time::Duration::from_secs(rand::thread_rng().gen_range(1..100));
        let tasks_number = rand::thread_rng().gen_range(1..100);
        let lag = tasks_number * catchup_interval.as_secs();
        let current_epoch = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
        let start_timestamp =  current_epoch - lag;
        let indexer_settings = IndexerSettings {
            concurrency: 1,
            //random catchup interval from 1 to 100
            catchup_interval,
            // current epoch - random from 1 to 100 times catchup interval
            start_timestamp,
            ..Default::default()
        };
        let settings = Settings::default("postgres://postgres:postgres@database:5432/blockscout".to_string());
        let server = init_tac_operation_lifecycle_server(db.db_url(), |settings| settings).await;
        let indexer = Indexer::new(indexer_settings, db.client()).await.unwrap();
        indexer.save_intervals().await.unwrap();
        let intervals = interval::Entity::find()
            .all(db.client().as_ref())
            .await.unwrap();
        assert_eq!(intervals.len(), tasks_number as usize);
        for i in 0..tasks_number as usize {
            let index = i as u64;
            assert_eq!(intervals[i].start as u64, start_timestamp + index * catchup_interval.as_secs());
            assert_eq!(intervals[i].end as u64, start_timestamp + (index + 1) * catchup_interval.as_secs());
        }

        assert_eq!(indexer.watermark(), current_epoch);
    
    }
}
