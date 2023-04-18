use crate::Client;
use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::{metadata, pending_addresses};
use sea_orm::{ActiveValue::Set, DbErr, EntityTrait};
use serde::Deserialize;
use tracing::instrument;

const OFFSET: usize = 1000;

#[derive(Clone, Debug, Deserialize)]
struct ResponseResult {
    #[serde(rename = "Address")]
    address: Bytes,
}

#[derive(Clone, Debug, Deserialize)]
struct Response {
    result: Vec<ResponseResult>,
}

#[instrument(name = "extract pending addresses", skip_all, err)]
pub async fn extract(client: Client) -> anyhow::Result<()> {
    let metadata_model = metadata::Entity::find()
        .one(client.db.as_ref())
        .await
        .context("extract addresses: last run timestamp extraction")?;

    let request = client.blockscout_client.get(client.blockscout_url).query(&[
        ("module", "contract"),
        ("action", "listcontracts"),
        ("filter", "verified"),
        ("offset", &format!("{OFFSET}")),
    ]);
    let request = match metadata_model {
        Some(metadata) if metadata.last_list_contracts_run.is_some() => {
            let last_list_contracts_run = metadata.last_list_contracts_run.unwrap();

            tracing::info!(
                "found last run: datetime={last_list_contracts_run}, timestamp={}",
                last_list_contracts_run.timestamp()
            );

            request.query(&[(
                "verified_at_start_timestamp",
                format!("{}", last_list_contracts_run.timestamp()),
            )])
        }
        _ => {
            tracing::info!("empty last run timestamp");
            request
        }
    };

    let mut page = 1;
    let last_request_timestamp = loop {
        let timestamp = chrono::Utc::now();
        let result = request
            .try_clone()
            .unwrap()
            .query(&[("page", format!("{page}"))])
            .send()
            .await
            .context(format!("extract addresses: api request; page={page}"))?
            .json::<Response>()
            .await
            .context(format!(
                "extract addresses: api response deserialization; page={page}"
            ))?
            .result;

        tracing::info!(
            "new pending addresses extraction: page={page}, offset={OFFSET}; extracted={}",
            result.len()
        );

        if result.is_empty() {
            break timestamp;
        }

        match pending_addresses::Entity::insert_many(result.into_iter().map(|contract| {
            pending_addresses::ActiveModel {
                address: Set(contract.address.to_vec()),
                ..Default::default()
            }
        }))
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(pending_addresses::Column::Address)
                .do_nothing()
                .to_owned(),
        )
        .exec(client.db.as_ref())
        .await
        {
            Ok(_) | Err(DbErr::RecordNotInserted) => {}
            Err(err) => {
                return Err(
                    anyhow::anyhow!(err).context("extract addresses: pending addresses insertion")
                )
            }
        };

        page += 1;
    };

    metadata::Entity::insert(metadata::ActiveModel {
        id: Set(1),
        last_list_contracts_run: Set(Some(last_request_timestamp.naive_utc())),
    })
    .on_conflict(
        sea_orm::sea_query::OnConflict::column(metadata::Column::Id)
            .update_column(metadata::Column::LastListContractsRun)
            .to_owned(),
    )
    .exec(client.db.as_ref())
    .await
    .context("extract addresses: last run timestamp insertion")?;

    tracing::info!(
        "updated last run: datetime={}, timestamp={}",
        last_request_timestamp.naive_utc(),
        last_request_timestamp.naive_utc().timestamp()
    );

    Ok(())
}
