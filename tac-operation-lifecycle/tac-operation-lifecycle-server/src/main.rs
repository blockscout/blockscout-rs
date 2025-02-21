use std::sync::Arc;

use blockscout_service_launcher::launcher::ConfigSettings;
use config::{Config, File};
use migration::Migrator;
use tac_operation_lifecycle_server::{Settings, run};
use tac_operation_lifecycle_logic::{Indexer, client::Client as TacClient};
use blockscout_service_launcher::{database};
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {


    // let service_name = "TAC_OPERATION_LIFECYCLE";
    // println!("Building config for {}", service_name);
    //     let config_path_name: &String = &format!("{}__CONFIG", service_name);
    //     let config_path = std::env::var(config_path_name);

    //     let mut builder = Config::builder();
    //     println!("config_path: {:?}", config_path);

    //     // println!("current env: {:?}", std::env::vars());

    //     println!("current folder: {:?}", std::env::current_dir());

    //     // print content of config_path
    //     if let Ok(ref config_path) = config_path {
    //         let content = std::fs::read_to_string(config_path).unwrap();
    //         println!("content of config_path: {}", content);
    //     }
        

    let settings = Settings::build().expect("failed to read config");
    

    let db_connection = database::initialize_postgres::<Migrator>(
        &settings.database,
    )
    .await?;
    // let client = TacClient::new(settings.indexer.map(|indexer| indexer.client).unwrap_or(default_client()));
    let indexer = Indexer::new(settings.clone().indexer.unwrap(), Arc::new(db_connection)).await?;

    indexer.start().await
    // run(settings).await
}
