use crate::{blockscout, eth_bytecode_db};
use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::{
    contract_addresses,
    sea_orm_active_enums::{Status, VerificationMethod},
    solidity_multiples, solidity_singles, solidity_standards, vyper_singles,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, EntityTrait, QueryFilter, Statement,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub blockscout_client: blockscout::Client,
    pub eth_bytecode_db_client: eth_bytecode_db::Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VerifiableContract {
    SoliditySingle(solidity_singles::Model),
    SolidityMultiple(solidity_multiples::Model),
    SolidityStandard(solidity_standards::Model),
    VyperSingle(vyper_singles::Model),
}

impl VerifiableContract {
    pub fn contract_address(&self) -> Bytes {
        match self {
            VerifiableContract::SoliditySingle(model) => {
                Bytes::from(model.contract_address.clone())
            }
            VerifiableContract::SolidityMultiple(model) => {
                Bytes::from(model.contract_address.clone())
            }
            VerifiableContract::SolidityStandard(model) => {
                Bytes::from(model.contract_address.clone())
            }
            VerifiableContract::VyperSingle(model) => Bytes::from(model.contract_address.clone()),
        }
    }
}

enum EthBytecodeDbRequest {
    SolidityMultiple(eth_bytecode_db::VerifySolidityMultiPartRequest),
    SolidityStandard(eth_bytecode_db::VerifySolidityStandardJsonRequest),
    #[allow(dead_code)]
    VyperMultiple(eth_bytecode_db::VerifyVyperMultiPartRequest),
}

impl EthBytecodeDbRequest {
    pub fn new(contract: VerifiableContract, creation_input: Bytes) -> anyhow::Result<Self> {
        let verification_metadata =
            |contract_address: Vec<u8>| eth_bytecode_db::VerificationMetadata {
                chain_id: "1".to_string(),
                contract_address: Bytes::from(contract_address).to_string(),
            };

        let request = match contract {
            VerifiableContract::SoliditySingle(model) => {
                EthBytecodeDbRequest::SolidityMultiple(eth_bytecode_db::VerifySolidityMultiPartRequest {
                    bytecode: creation_input.to_string(),
                    bytecode_type: eth_bytecode_db::BytecodeType::CreationInput.into(),
                    compiler_version: model.compiler_version,
                    evm_version: None,
                    optimization_runs: model.optimizations.then_some(model.optimization_runs as i32),
                    source_files: BTreeMap::from([("main.sol".to_string(), model.source_code)]),
                    libraries: Default::default(),
                    metadata: Some(verification_metadata(model.contract_address)),
                })
            }
            VerifiableContract::SolidityMultiple(model) => {
                EthBytecodeDbRequest::SolidityMultiple(eth_bytecode_db::VerifySolidityMultiPartRequest {
                    bytecode: creation_input.to_string(),
                    bytecode_type: eth_bytecode_db::BytecodeType::CreationInput.into(),
                    compiler_version: model.compiler_version,
                    evm_version: None,
                    optimization_runs: model.optimizations.then_some(model.optimization_runs as i32),
                    source_files: serde_json::from_value(model.sources)
                        .context("solidity multiple model (conversion to request) source files deserialization failed")?,
                    libraries: Default::default(),
                    metadata: Some(verification_metadata(model.contract_address)),
                })
            }
            VerifiableContract::SolidityStandard(model) => {
                EthBytecodeDbRequest::SolidityStandard(eth_bytecode_db::VerifySolidityStandardJsonRequest {
                    bytecode: creation_input.to_string(),
                    bytecode_type: eth_bytecode_db::BytecodeType::CreationInput.into(),
                    compiler_version: model.compiler_version,
                    input: model.standard_json.to_string(),
                    metadata: Some(verification_metadata(model.contract_address)),
                })
            }
            VerifiableContract::VyperSingle(_model) => {
                return Err(anyhow::anyhow!("vyper contracts cannot be processed yet"))
            }
        };

        Ok(request)
    }

    pub async fn verify(self, client: &mut Client) -> anyhow::Result<eth_bytecode_db::Source> {
        let response = match self {
            EthBytecodeDbRequest::SolidityMultiple(request) => {
                client
                    .eth_bytecode_db_client
                    .verify_solidity_multi_part(request)
                    .await
            }
            EthBytecodeDbRequest::SolidityStandard(request) => {
                client
                    .eth_bytecode_db_client
                    .verify_solidity_standard_json(request)
                    .await
            }
            EthBytecodeDbRequest::VyperMultiple(request) => {
                client
                    .eth_bytecode_db_client
                    .verify_vyper_multi_part(request)
                    .await
            }
        }
        .context("sending verification request failed")?;
        if let eth_bytecode_db::verify_response::Status::Success = response.status() {
            Ok(response.source.unwrap())
        } else {
            Err(anyhow::anyhow!(
                "contract verification failed with message: {}",
                response.message
            ))
        }
    }
}

impl Client {
    pub async fn try_new_arc(
        db: Arc<DatabaseConnection>,
        blockscout_url: String,
        etherscan_url: String,
        etherscan_api_key: String,
        etherscan_limit_requests_per_second: u32,
        eth_bytecode_db_url: String,
    ) -> anyhow::Result<Self> {
        let blockscout_client = blockscout::Client::try_new(
            blockscout_url,
            etherscan_url,
            etherscan_api_key,
            etherscan_limit_requests_per_second,
        )?;

        let eth_bytecode_db_client = eth_bytecode_db::Client::try_new(eth_bytecode_db_url)?;

        let client = Self {
            db_client: db,
            blockscout_client,
            eth_bytecode_db_client,
        };
        client.reset_database().await?;

        Ok(client)
    }

    pub async fn verify_contracts(mut self) -> anyhow::Result<()> {
        macro_rules! process_result {
            ( $result:expr, $contract_address:expr ) => {
                match $result {
                    Ok(res) => res,
                    Err(err) => {
                        contract_addresses::ActiveModel {
                            contract_address: Set($contract_address.to_vec()),
                            status: Set(Status::Error),
                            log: Set(Some(format!("{:#?}", err))),
                            ..Default::default()
                        }
                        .update(self.db_client.as_ref())
                        .await
                        .context(format!(
                            "saving error details failed for the contract {}",
                            $contract_address,
                        ))?;

                        continue;
                    }
                }
            };
        }

        while let Some(next_contract) = self.next_contract().await? {
            let contract_address = next_contract.contract_address();

            let creation_input = process_result!(
                self.extract_creation_input(contract_address.clone()).await,
                contract_address.clone()
            );

            let request = process_result!(
                EthBytecodeDbRequest::new(next_contract, creation_input),
                contract_address.clone()
            );
            let source = process_result!(request.verify(&mut self).await, contract_address.clone());
            self.mark_as_success(contract_address, source).await?;
        }
        Ok(())
    }

    pub async fn search_contracts(self) -> anyhow::Result<()> {
        loop {
            let sql = r#"
            SELECT contract_address, creation_input
            FROM contract_addresses
            WHERE status = 'success'
            ORDER BY random()
            LIMIT 1;
        "#;
            let stmt = Statement::from_string(DatabaseBackend::Postgres, sql.to_string());
            let result = self
                .db_client
                .as_ref()
                .query_one(stmt)
                .await
                .context("querying for the success contract address and creation input")?
                .map(|query_result| {
                    let contract_address = query_result
                        .try_get_by::<Vec<u8>, _>("contract_address")
                        .expect("error while try_get_by contract_address");
                    let creation_input = query_result
                        .try_get_by::<Option<Vec<u8>>, _>("creation_input")
                        .expect("error while try_get_by creation_input");
                    (contract_address, creation_input)
                });

            if let Some((contract_address, creation_input)) = result {
                tracing::info!(
                    "search contract_address: {}",
                    Bytes::from(contract_address.clone())
                );
                let request = eth_bytecode_db::SearchSourcesRequest {
                    bytecode: Bytes::from(creation_input.unwrap()).to_string(),
                    bytecode_type: eth_bytecode_db::BytecodeType::CreationInput.into(),
                };
                let search_result = self.eth_bytecode_db_client.search_sources(request).await;
                if let Err(err) = search_result {
                    tracing::info!("{err:#?}")
                }
            }
            // tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    /// Reset all `in_process` contracts back to the `waiting` state.
    /// Should be called on the client initialization in order to reset
    /// previously non-finished tasks back to the state where they can be processed again.
    async fn reset_database(&self) -> anyhow::Result<()> {
        let active_model = contract_addresses::ActiveModel {
            status: Set(Status::Waiting),
            ..Default::default()
        };
        contract_addresses::Entity::update_many()
            .filter(contract_addresses::Column::Status.eq(Status::InProcess))
            .set(active_model)
            .exec(self.db_client.as_ref())
            .await
            .context("resetting database failed")?;

        Ok(())
    }

    async fn next_contract(&self) -> anyhow::Result<Option<VerifiableContract>> {
        macro_rules! verifiable_contract_model {
            ( $entity_module:ident, $contract_address:expr ) => {
                $entity_module::Entity::find_by_id($contract_address.clone())
                    .one(self.db_client.as_ref())
                    .await
                    .context(format!("querying {} model", stringify!($entity_module)))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{} model does not exist for the contract: {}",
                            stringify!($entity_module),
                            blockscout_display_bytes::Bytes::from($contract_address),
                        )
                    })?
            };
        }

        let next_contract_address_sql = r#"
            UPDATE contract_addresses
            SET status = 'in_process'
            WHERE contract_address = (SELECT contract_address
                                      FROM contract_addresses
                                      WHERE status = 'waiting'
                                      LIMIT 1 FOR UPDATE SKIP LOCKED)
            RETURNING contract_address;
        "#;
        let next_contract_address_stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            next_contract_address_sql.to_string(),
        );
        let contract_address = self
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

        if let Some(contract_address) = contract_address {
            let contract_address_model =
                contract_addresses::Entity::find_by_id(contract_address.clone())
                    .one(self.db_client.as_ref())
                    .await
                    .context("querying contract_address model")?
                    .unwrap();

            let next_contract = match contract_address_model.verification_method {
                VerificationMethod::SoliditySingle => {
                    let model = verifiable_contract_model!(solidity_singles, contract_address);
                    VerifiableContract::SoliditySingle(model)
                }
                VerificationMethod::SolidityMultiple => {
                    let model = verifiable_contract_model!(solidity_multiples, contract_address);
                    VerifiableContract::SolidityMultiple(model)
                }
                VerificationMethod::SolidityStandard => {
                    let model = verifiable_contract_model!(solidity_standards, contract_address);
                    VerifiableContract::SolidityStandard(model)
                }
                VerificationMethod::VyperSingle => {
                    let model = verifiable_contract_model!(vyper_singles, contract_address);
                    VerifiableContract::VyperSingle(model)
                }
            };

            return Ok(Some(next_contract));
        }

        Ok(None)
    }

    async fn extract_creation_input(&self, contract_address: Bytes) -> anyhow::Result<Bytes> {
        let creation_input_opt = contract_addresses::Entity::find_by_id(contract_address.to_vec())
            .one(self.db_client.as_ref())
            .await
            .context("querying contract_address model")?
            .unwrap()
            .creation_input;

        if let Some(creation_input) = creation_input_opt {
            return Ok(Bytes::from(creation_input));
        }

        let creation_tx_hash = self
            .blockscout_client
            .get_contract_creation_transaction(contract_address.clone())
            .await
            .context(format!(
                "get_contract_creation_transaction({})",
                contract_address
            ))?;
        let creation_input = self
            .blockscout_client
            .get_transaction_input(creation_tx_hash.clone())
            .await
            .context(format!("get_transaction_input({})", creation_tx_hash))?;

        contract_addresses::ActiveModel {
            contract_address: Set(contract_address.to_vec()),
            creation_input: Set(Some(creation_input.to_vec())),
            ..Default::default()
        }
        .update(self.db_client.as_ref())
        .await
        .context("updating contract_address model to insert creation input")?;

        Ok(creation_input)
    }

    pub async fn mark_as_success(
        &self,
        contract_address: Bytes,
        source: eth_bytecode_db::Source,
    ) -> anyhow::Result<()> {
        contract_addresses::ActiveModel {
            contract_address: Set(contract_address.to_vec()),
            status: Set(Status::Success),
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
}
