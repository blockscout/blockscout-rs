use crate::verification::handlers::process_abi_data;
use anyhow::Context;
use entity::sources;
use futures::TryStreamExt;
use sea_orm::{DatabaseConnection, EntityTrait, FromQueryResult, QuerySelect};

pub async fn import(db_client: &DatabaseConnection) {
    #[derive(FromQueryResult)]
    struct Abi {
        pub abi: sea_orm::JsonValue,
    }

    let mut stream = sources::Entity::find()
        .select_only()
        .column(sources::Column::Abi)
        .distinct()
        .into_model::<Abi>()
        .stream(db_client)
        .await
        .expect("creating a stream");

    let mut processed = 0;

    loop {
        match stream.try_next().await.context("getting next abi value") {
            Ok(None) => break,
            Ok(Some(item)) => {
                let abi = item.abi.to_string();
                if let Err(err) = process_abi_data(Some(abi.clone()), db_client).await {
                    println!("[ERROR] Error while processing abi; abi={abi}, err={err:#}");
                }
            }
            Err(err) => {
                println!("[ERROR] Error while processing next abi: {err}");
            }
        }
        processed += 1;

        if processed % 100 == 0 {
            println!("Processed={processed}")
        }
    }
    println!("\nabis processed successfully; total={processed}");
}
