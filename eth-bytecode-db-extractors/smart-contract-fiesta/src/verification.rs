use crate::blockscout;
use anyhow::Context;
use entity::{
    contract_addresses, sea_orm_active_enums::VerificationMethod, solidity_multiples,
    solidity_singles, solidity_standards, vyper_singles,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, Statement};
use std::sync::Arc;

pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub blockscout_client: blockscout::Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VerifiableContract {
    SoliditySingle(solidity_singles::Model),
    SolidityMultiple(solidity_multiples::Model),
    SolidityStandard(solidity_standards::Model),
    VyperSingle(vyper_singles::Model),
}

impl Client {
    pub fn try_new_arc(
        db: Arc<DatabaseConnection>,
        blockscout_url: String,
    ) -> anyhow::Result<Self> {
        let blockscout_client = blockscout::Client::try_new(blockscout_url)?;

        Ok(Self {
            db_client: db,
            blockscout_client,
        })
    }

    pub async fn verify_contracts(&self) -> anyhow::Result<()> {
        while let Some(_next_contract) = self.next_contract().await? {}

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
}
