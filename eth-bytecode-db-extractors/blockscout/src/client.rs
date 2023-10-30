use crate::{blockscout, eth_bytecode_db};
use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::{contract_addresses, contract_details, sea_orm_active_enums};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::{
    BytecodeType, Source, VerificationMetadata, VerifySolidityStandardJsonRequest,
    VerifyVyperStandardJsonRequest,
};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DbErr, EntityTrait, Statement,
};
use serde::Serialize;
use std::sync::Arc;

macro_rules! process_result {
    ( $result:expr, $self:expr, $contract_address:expr) => {
        match $result {
            Ok(res) => res,
            Err(err) => {
                tracing::warn!(
                    contract_address = $contract_address.to_string(),
                    error = format!("{err:#}"),
                    "Error processing contract"
                );

                contract_addresses::ActiveModel {
                    contract_address: Set($contract_address.to_vec()),
                    chain_id: Set($self.chain_id.into()),
                    status: Set(sea_orm_active_enums::Status::Error),
                    log: Set(Some(format!("{:#?}", err))),
                    ..Default::default()
                }
                .update($self.db_client.as_ref())
                .await
                .context(format!(
                    "saving error details failed; contract={}, chain_id={}",
                    $contract_address, $self.chain_id,
                ))?;

                continue;
            }
        }
    };
}

#[derive(Debug, Serialize)]
struct StandardJson {
    language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    interfaces: Option<serde_json::Value>,
    sources: serde_json::Value,
    settings: serde_json::Value,
}

#[derive(Clone)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub chain_id: u64,
    pub blockscout_client: blockscout::Client,
    pub eth_bytecode_db_client: eth_bytecode_db::Client,
}

impl Client {
    pub fn try_new(
        db_client: DatabaseConnection,
        chain_id: u64,
        blockscout_url: String,
        eth_bytecode_db_url: String,
        eth_bytecode_db_api_key: String,
        limit_requests_per_second: u32,
    ) -> anyhow::Result<Self> {
        let blockscout_client =
            blockscout::Client::try_new(blockscout_url, limit_requests_per_second)?;

        let eth_bytecode_db_client =
            eth_bytecode_db::Client::try_new(eth_bytecode_db_url, eth_bytecode_db_api_key)?;

        let client = Self {
            db_client: Arc::new(db_client),
            chain_id,
            blockscout_client,
            eth_bytecode_db_client,
        };

        Ok(client)
    }
}

impl Client {
    pub async fn import_contract_addresses(&self, force_import: bool) -> anyhow::Result<usize> {
        let mut verified_contracts = self
            .blockscout_client
            .get_verified_contracts()
            .await
            .context("get list of verified contracts")?;

        let mut processed = 0;
        while let Some(items) = verified_contracts.next_page().await.context(format!(
            "extracting contract addresses failed: items_count={:?}, smart_contract_id={:?}",
            verified_contracts.items_count(),
            verified_contracts.smart_contract_id()
        ))? {
            processed += items.len();
            if processed % 200 == 0 {
                tracing::info!(
                    "Processed={processed}, next_page_smart_contract_id={:?}",
                    verified_contracts.smart_contract_id()
                );
            }

            let address_models = items
                .iter()
                .map(|item| {
                    let language = match item.language.as_ref() {
                        "solidity" => sea_orm_active_enums::Language::Solidity,
                        "yul" => sea_orm_active_enums::Language::Yul,
                        "vyper" => sea_orm_active_enums::Language::Vyper,
                        language => return Err(anyhow::anyhow!("Invalid language: {language}")),
                    };
                    Ok(contract_addresses::ActiveModel {
                        contract_address: Set(item.address.to_vec()),
                        chain_id: Set(self.chain_id.into()),
                        verified_at: Set(item.verified_at),
                        language: Set(language),
                        compiler_version: Set(item.compiler_version.clone()),
                        ..Default::default()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            match contract_addresses::Entity::insert_many(address_models)
                .on_conflict(OnConflict::new().do_nothing().to_owned())
                .exec(self.db_client.as_ref())
                .await
            {
                Ok(_) => {}
                Err(DbErr::RecordNotInserted) => {
                    // Do not stop if re-import of all contracts have been setup.
                    if !force_import {
                        tracing::info!("No records have been inserted. Stop dataset import");
                        break;
                    }
                }
                Err(err) => {
                    return Err(err).context(format!(
                    "inserting contract addresses failed: items_count={:?}, smart_contract_id={:?}",
                    verified_contracts.items_count(),
                    verified_contracts.smart_contract_id()
                ))
                }
            }
        }

        Ok(processed)
    }

    pub async fn verify_contracts(self) -> anyhow::Result<usize> {
        let mut processed = 0;
        while let Some(contract_model) = self.next_contract().await? {
            processed += 1;
            let contract_address = Bytes::from(contract_model.contract_address.clone());

            tracing::info!(
                contract_address = contract_address.to_string(),
                "contract processed"
            );

            let contract_details_model = process_result!(
                self.import_contract_details(contract_address.clone()).await,
                &self,
                contract_address
            );

            let source = process_result!(
                self.verify_contract(contract_model, contract_details_model)
                    .await,
                &self,
                contract_address
            );
            self.mark_as_success(contract_address, source).await?;
        }

        Ok(processed)
    }

    async fn import_contract_details(
        &self,
        contract_address: Bytes,
    ) -> anyhow::Result<contract_details::Model> {
        let contract_details = self
            .blockscout_client
            .get_contract_details(contract_address.clone())
            .await
            .context("getting contract details failed")?;

        let contract_details_model = contract_details::ActiveModel {
            contract_address: Set(contract_address.to_vec()),
            chain_id: Set(self.chain_id.into()),
            sources: Set(contract_details.sources),
            settings: Set(contract_details.settings),
            verified_via_sourcify: Set(contract_details.verified_via_sourcify),
            optimization_enabled: Set(contract_details.optimization_enabled),
            optimization_runs: Set(contract_details.optimization_runs),
            evm_version: Set(contract_details.evm_version),
            libraries: Set(contract_details.libraries),
            creation_code: Set(contract_details.creation_code),
            runtime_code: Set(contract_details.runtime_code),
            transaction_hash: Set(Some(contract_details.transaction_hash)),
            block_number: Set(contract_details.block_number.into()),
            transaction_index: Set(contract_details.transaction_index.map(|index| index.into())),
            deployer: Set(Some(contract_details.deployer)),
            ..Default::default()
        }
        .insert(self.db_client.as_ref())
        .await
        .context("updating contract_details model to insert contract details")?;

        Ok(contract_details_model)
    }

    async fn verify_contract(
        &self,
        contract: contract_addresses::Model,
        contract_details: contract_details::Model,
    ) -> anyhow::Result<Source> {
        let input = if contract_details.verified_via_sourcify {
            self.generate_input_from_sourcify().await?
        } else if let Some(_libraries) = contract_details.libraries {
            Self::generate_input_with_libraries()?
        } else {
            Self::generate_input(contract.language.clone(), &contract_details)?
        };

        let (bytecode, bytecode_type) =
            if let Some(creation_code) = contract_details.creation_code.as_ref() {
                (creation_code.clone(), BytecodeType::CreationInput)
            } else {
                (
                    contract_details.runtime_code.clone(),
                    BytecodeType::DeployedBytecode,
                )
            };

        let vec_to_string = |vec: Vec<u8>| Bytes::from(vec).to_string();

        let metadata = VerificationMetadata {
            chain_id: Some(format!("{}", self.chain_id)),
            contract_address: Some(vec_to_string(contract.contract_address)),
            transaction_hash: contract_details.transaction_hash.map(vec_to_string),
            block_number: Some(contract_details.block_number.try_into().unwrap()),
            transaction_index: contract_details
                .transaction_index
                .map(|v| v.try_into().unwrap()),
            deployer: contract_details.deployer.map(vec_to_string),
            creation_code: contract_details.creation_code.map(vec_to_string),
            runtime_code: Some(vec_to_string(contract_details.runtime_code)),
        };

        macro_rules! send_eth_bytecode_db_request {
            ($request_type:tt, $verify:tt) => {{
                let request = $request_type {
                    bytecode: Bytes::from(bytecode).to_string(),
                    bytecode_type: bytecode_type.into(),
                    compiler_version: contract.compiler_version,
                    input: serde_json::to_string(&input)
                        .context("serializing standard json input failed")?,
                    metadata: Some(metadata),
                };

                self.eth_bytecode_db_client.$verify(request).await
            }};
        }

        let source = match contract.language {
            sea_orm_active_enums::Language::Solidity | sea_orm_active_enums::Language::Yul => {
                send_eth_bytecode_db_request!(
                    VerifySolidityStandardJsonRequest,
                    verify_solidity_standard_json
                )
            }
            sea_orm_active_enums::Language::Vyper => {
                send_eth_bytecode_db_request!(
                    VerifyVyperStandardJsonRequest,
                    verify_vyper_standard_json
                )
            }
        }
        .context("verify through eth_bytecode_db failed")?;

        Ok(source)
    }

    fn generate_input(
        language: sea_orm_active_enums::Language,
        contract_details: &contract_details::Model,
    ) -> anyhow::Result<StandardJson> {
        let (language, interfaces) = match language {
            sea_orm_active_enums::Language::Solidity => ("Solidity", None),
            sea_orm_active_enums::Language::Yul => ("Yul", None),
            sea_orm_active_enums::Language::Vyper => ("Vyper", Some(serde_json::json!({}))),
        };

        let settings = if let Some(settings) = &contract_details.settings {
            settings.clone()
        } else {
            #[derive(Debug, Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Settings {
                pub optimizer: Optimizer,
                pub evm_version: Option<String>,
            }

            #[derive(Debug, Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Optimizer {
                pub enabled: Option<bool>,
                pub runs: Option<i64>,
            }

            let settings = Settings {
                optimizer: Optimizer {
                    enabled: contract_details.optimization_enabled,
                    runs: contract_details.optimization_runs,
                },
                evm_version: contract_details
                    .evm_version
                    .clone()
                    .filter(|v| v != "default"),
            };

            serde_json::to_value(settings).unwrap()
        };

        Ok(StandardJson {
            language: language.to_string(),
            sources: contract_details.sources.clone(),
            interfaces,
            settings,
        })
    }

    fn generate_input_with_libraries() -> anyhow::Result<StandardJson> {
        Err(anyhow::anyhow!(
            "Input generation for sources with libraries is not implemented yet"
        ))
    }

    async fn generate_input_from_sourcify(&self) -> anyhow::Result<StandardJson> {
        Err(anyhow::anyhow!(
            "Input generation from sourcify is not implemented yet"
        ))
    }

    async fn mark_as_success(
        &self,
        contract_address: Bytes,
        source: eth_bytecode_db::Source,
    ) -> anyhow::Result<()> {
        contract_addresses::ActiveModel {
            contract_address: Set(contract_address.to_vec()),
            chain_id: Set(self.chain_id.into()),
            status: Set(sea_orm_active_enums::Status::Success),
            log: Set(Some(
                serde_json::to_string(&source)
                    .context("serializing success result (source) failed")?,
            )),
            ..Default::default()
        }
        .update(self.db_client.as_ref())
        .await
        .context(format!(
            "saving success details failed for the contract {}",
            contract_address,
        ))?;

        Ok(())
    }

    async fn next_contract(&self) -> anyhow::Result<Option<contract_addresses::Model>> {
        // Notice that we are looking only for contracts with given `chain_id`
        let next_contract_address_sql = format!(
            r#"
            UPDATE contract_addresses
            SET status = 'in_process'
            WHERE contract_address = (SELECT contract_address
                                      FROM contract_addresses
                                      WHERE status = 'waiting'
                                        AND chain_id = {}
                                      LIMIT 1 FOR UPDATE SKIP LOCKED)
            RETURNING contract_address;
        "#,
            self.chain_id
        );

        let next_contract_address_stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            next_contract_address_sql.to_string(),
        );

        let next_contract_address = self
            .db_client
            .as_ref()
            .query_one(next_contract_address_stmt)
            .await
            .context("querying for the next contract address")?
            .map(|query_result| {
                query_result
                    .try_get_by::<Vec<u8>, _>("contract_address")
                    .expect("error while try_get_by contract_address")
            });

        if let Some(contract_address) = next_contract_address {
            let model = contract_addresses::Entity::find_by_id((
                contract_address.clone(),
                self.chain_id.into(),
            ))
            .one(self.db_client.as_ref())
            .await
            .expect("querying contract_address model failed")
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "contract_address model does not exist for the contract: {}",
                    Bytes::from(contract_address),
                )
            })?;

            return Ok(Some(model));
        }

        Ok(None)
    }
}
