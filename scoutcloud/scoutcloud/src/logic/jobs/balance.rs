#![allow(clippy::blocks_in_conditions)]

use super::{global, StoppingTask};
use crate::logic::DeployError;
use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};
use scoutcloud_entity as db;
use sea_orm::{
    prelude::*, ActiveValue::Set, DbBackend, FromQueryResult, IsolationLevel, Statement,
    TransactionTrait,
};
use std::ops::Mul;
use tracing::instrument;

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Default)]
#[serde(crate = "fang::serde")]
pub struct CheckBalanceTask {}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for CheckBalanceTask {
    #[instrument(err(Debug), skip(self, client))]
    async fn run(&self, client: &mut dyn AsyncQueueable) -> Result<(), FangError> {
        let db = global::get_db_connection();
        // 'check balance' is unique task and there is only one instance of this task running
        // however we begin transaction with serializable isolation level just in case
        let tx = db
            .begin_with_config(Some(IsolationLevel::Serializable), None)
            .await
            .map_err(DeployError::Db)?;
        let unpaid_list = UnPaidDeployment::all(&tx).await.map_err(DeployError::Db)?;
        if unpaid_list.is_empty() {
            return Ok(());
        }
        tracing::info!(unpaid = ?unpaid_list, "found {} unpaid deployments. start processing", unpaid_list.len());
        for unpaid in unpaid_list {
            if unpaid.creator_can_pay(&tx).await.map_err(DeployError::Db)? {
                unpaid.mark_as_paid(&tx).await.map_err(DeployError::Db)?;
            } else {
                tracing::warn!(
                    user_id = unpaid.creator_id,
                    deployment_id = unpaid.deployment_id,
                    "user can't pay for deployment. stopping deployment",
                );
                // TODO: notify user
                client
                    .insert_task(&StoppingTask::new(unpaid.deployment_id))
                    .await?;
            }
        }
        tx.commit().await.map_err(DeployError::Db)?;
        Ok(())
    }

    fn uniq(&self) -> bool {
        true
    }

    fn cron(&self) -> Option<Scheduled> {
        Some(Scheduled::CronPattern("0 * * * * *".to_string()))
    }
}

#[derive(Debug, FromQueryResult)]
struct UnPaidDeployment {
    deployment_id: i32,
    total_used_hours: i32,
    total_paid_hours: i32,
    cost_per_hour: Decimal,
    creator_id: i32,
}

impl UnPaidDeployment {
    pub fn hours(&self) -> i32 {
        self.total_used_hours - self.total_paid_hours
    }

    pub fn expense_amount(&self) -> Decimal {
        self.cost_per_hour.mul(Decimal::new(self.hours() as i64, 0))
    }

    pub async fn all(db: &impl ConnectionTrait) -> Result<Vec<Self>, DbErr> {
        let select = r#"
        SELECT
            unpaid_deployments.id as deployment_id,
            unpaid_deployments.total_used_hours::INT4,
            unpaid_deployments.total_paid_hours::INT4,
            server_specs.cost_per_hour,
            instances.creator_id
        FROM (
            SELECT
                deployments.id,
                deployments.server_spec_id,
                deployments.instance_id,
                CEIL(EXTRACT(EPOCH FROM (COALESCE(deployments.finished_at, CURRENT_TIMESTAMP) - deployments.created_at)) / 3600) AS total_used_hours,
                COALESCE(SUM(balance_expenses.hours), 0) AS total_paid_hours
            FROM
                deployments
            LEFT JOIN
                balance_expenses ON deployments.id = balance_expenses.deployment_id

            WHERE
                deployments.status = 'running'
            GROUP BY
                deployments.id
             ) unpaid_deployments
        LEFT JOIN server_specs ON unpaid_deployments.server_spec_id = server_specs.id
        LEFT JOIN instances ON unpaid_deployments.instance_id = instances.id
        WHERE total_used_hours > total_paid_hours
        "#;

        UnPaidDeployment::find_by_statement(Statement::from_string(DbBackend::Postgres, select))
            .all(db)
            .await
    }

    pub async fn creator_can_pay(&self, db: &impl ConnectionTrait) -> Result<bool, DbErr> {
        let user = db::users::Entity::find_by_id(self.creator_id)
            .one(db)
            .await?
            .ok_or(DbErr::Custom("deployment creator not found".into()))?;
        Ok(user.balance >= self.expense_amount())
    }

    pub async fn mark_as_paid(&self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        let hours = self.hours();
        let expense_amount = self.expense_amount();
        let expense = db::balance_expenses::ActiveModel {
            user_id: Set(self.creator_id),
            deployment_id: Set(self.deployment_id),
            hours: Set(hours),
            expense_amount: Set(expense_amount),
            ..Default::default()
        };
        expense.save(db).await?;
        Ok(())
    }
}
