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

#[derive(fang::serde::Serialize, fang::serde::Deserialize)]
#[serde(crate = "fang::serde")]
pub struct CheckBalanceTask {
    schedule: Option<String>,
    #[cfg(test)]
    database_url: Option<String>,
}

impl Default for CheckBalanceTask {
    fn default() -> Self {
        Self {
            schedule: Some("0 * * * * *".to_string()),
            #[cfg(test)]
            database_url: None,
        }
    }
}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for CheckBalanceTask {
    #[instrument(err(Debug), skip(self, client), level = "info")]
    async fn run(&self, client: &dyn AsyncQueueable) -> Result<(), FangError> {
        let db = global::DATABASE.get().await;
        // 'check balance' is unique task and there is only one instance of this task running
        // however we begin transaction with serializable isolation level just in case
        let tx = db
            .begin_with_config(Some(IsolationLevel::Serializable), None)
            .await
            .map_err(DeployError::Db)?;
        let unpaid_list = UnpaidDeployment::all(&tx).await.map_err(DeployError::Db)?;
        if unpaid_list.is_empty() {
            return Ok(());
        }
        tracing::info!(unpaid = ?unpaid_list, "found {} unpaid deployments. start processing", unpaid_list.len());
        for unpaid in unpaid_list {
            if unpaid.creator_can_pay(&tx).await.map_err(DeployError::Db)? {
                unpaid.mark_as_paid(&tx).await.map_err(DeployError::Db)?;
            } else {
                tracing::info!(
                    user_id = unpaid.creator_id,
                    deployment_id = unpaid.deployment_id,
                    "user can't pay for deployment. stopping deployment",
                );
                // create expense in any case, user balance will be negative
                unpaid.mark_as_paid(&tx).await.map_err(DeployError::Db)?;
                // TODO: maybe notify user?
                client
                    .insert_task(&StoppingTask::from_deployment_id(unpaid.deployment_id))
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
        self.schedule
            .as_ref()
            .map(|s| Scheduled::CronPattern(s.clone()))
    }
}

#[derive(Debug, FromQueryResult, PartialEq, Eq)]
struct UnpaidDeployment {
    deployment_id: i32,
    total_used_hours: i32,
    total_paid_hours: i32,
    cost_per_hour: Decimal,
    creator_id: i32,
}

impl UnpaidDeployment {
    pub fn hours(&self) -> i32 {
        self.total_used_hours - self.total_paid_hours
    }

    pub fn expense_amount(&self) -> Decimal {
        self.cost_per_hour.mul(Decimal::new(self.hours() as i64, 0))
    }

    pub async fn all<C>(db: &C) -> Result<Vec<Self>, DbErr>
    where
        C: ConnectionTrait,
    {
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
                CEIL(EXTRACT(EPOCH FROM (COALESCE(deployments.finished_at, CURRENT_TIMESTAMP) - deployments.started_at)) / 3600) AS total_used_hours,
                COALESCE(SUM(balance_expenses.hours), 0) AS total_paid_hours
            FROM
                deployments
            LEFT JOIN
                balance_expenses ON deployments.id = balance_expenses.deployment_id
            WHERE
                deployments.started_at IS NOT NULL
                AND deployments.status != 'failed'
            GROUP BY
                deployments.id
             ) unpaid_deployments
        LEFT JOIN server_specs ON unpaid_deployments.server_spec_id = server_specs.id
        LEFT JOIN instances ON unpaid_deployments.instance_id = instances.id
        WHERE total_used_hours > total_paid_hours
        ORDER BY deployment_id
        "#;

        UnpaidDeployment::find_by_statement(Statement::from_string(DbBackend::Postgres, select))
            .all(db)
            .await
    }

    pub async fn creator_can_pay<C>(&self, db: &C) -> Result<bool, DbErr>
    where
        C: ConnectionTrait,
    {
        let user = db::users::Entity::find_by_id(self.creator_id)
            .one(db)
            .await?
            .ok_or(DbErr::Custom("deployment creator not found".into()))?;
        Ok(user.balance >= self.expense_amount())
    }

    pub async fn mark_as_paid<C>(&self, db: &C) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{logic::jobs::balance::UnpaidDeployment, tests_utils};
    use pretty_assertions::assert_eq;
    use sea_orm::ActiveValue::Set;

    #[tokio::test]
    #[serial_test::serial]
    async fn select_unpaid_deployments_works() {
        let (db, _github, _repo, _runner) =
            tests_utils::init::jobs_runner_test_case("select_unpaid_deployments_works").await;
        let conn = db.client();

        // just in case delete
        scoutcloud_entity::balance_expenses::Entity::delete_many()
            .exec(conn.as_ref())
            .await
            .unwrap();
        let list = UnpaidDeployment::all(conn.as_ref()).await.unwrap();
        let cost_small = Decimal::new(1, 0);
        assert_eq!(
            list,
            vec![
                UnpaidDeployment {
                    deployment_id: 1,
                    total_used_hours: 5,
                    total_paid_hours: 0,
                    cost_per_hour: cost_small,
                    creator_id: 1,
                },
                UnpaidDeployment {
                    deployment_id: 2,
                    total_used_hours: 4,
                    total_paid_hours: 0,
                    cost_per_hour: cost_small,
                    creator_id: 2,
                },
            ]
        );
        // fully pay for deployment#2
        scoutcloud_entity::balance_expenses::ActiveModel {
            user_id: Set(1),
            deployment_id: Set(2),
            hours: Set(4),
            expense_amount: Set(Default::default()), // doesn't matter in this test
            ..Default::default()
        }
        .insert(conn.as_ref())
        .await
        .unwrap();

        // partially pay for deployment#1
        let paid_hours = 2;
        scoutcloud_entity::balance_expenses::ActiveModel {
            user_id: Set(2),
            deployment_id: Set(1),
            hours: Set(paid_hours),
            expense_amount: Set(Default::default()),
            ..Default::default()
        }
        .insert(conn.as_ref())
        .await
        .unwrap();

        let list = UnpaidDeployment::all(conn.as_ref()).await.unwrap();
        assert_eq!(
            list,
            vec![UnpaidDeployment {
                deployment_id: 1,
                total_used_hours: 5,
                total_paid_hours: paid_hours,
                cost_per_hour: cost_small,
                creator_id: 1,
            },]
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn check_balance_works() {
        let (db, _github, _repo, runner) =
            tests_utils::init::jobs_runner_test_case("check_balance_works").await;
        let conn = db.client();

        scoutcloud_entity::balance_expenses::Entity::delete_many()
            .exec(conn.as_ref())
            .await
            .unwrap();

        let task = CheckBalanceTask {
            schedule: None,
            database_url: Some(db.db_url().to_string()),
        };
        let n = 2;
        assert_eq!(UnpaidDeployment::all(conn.as_ref()).await.unwrap().len(), n);
        runner.insert_task(&task).await.unwrap();
        tests_utils::db::wait_for_empty_fang_tasks(conn.clone())
            .await
            .unwrap();
        let expenses_found = scoutcloud_entity::balance_expenses::Entity::find()
            .count(conn.as_ref())
            .await
            .unwrap();
        assert_eq!(expenses_found, n as u64);
        assert_eq!(UnpaidDeployment::all(conn.as_ref()).await.unwrap().len(), 0);
    }
}
