// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::auth::Actor;
use interchain_indexer_logic::{InterchainDatabase, write_api};
use sea_orm::{DatabaseTransaction, DbErr, TransactionTrait};

/// A transaction that commits a change together with its audit row.
///
/// The only way to commit is [`AuditedTx::commit`], which inserts the audit row
/// first, so an audited commit requires an [`Actor`] (who only
/// `WriteApiAuth::authenticate` creates). Dropping without `commit` rolls the
/// transaction back, and so does a failed audit insert.
pub(crate) struct AuditedTx<'a> {
    tx: DatabaseTransaction,
    actor: &'a Actor,
    method: &'static str,
    reason: String,
    request: serde_json::Value,
}

impl<'a> AuditedTx<'a> {
    pub(crate) async fn begin(
        db: &InterchainDatabase,
        actor: &'a Actor,
        method: &'static str,
        reason: String,
        request: serde_json::Value,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            tx: db.db.begin().await?,
            actor,
            method,
            reason,
            request,
        })
    }

    pub(crate) fn tx(&self) -> &DatabaseTransaction {
        &self.tx
    }

    /// Inserts the audit row, then commits; returns the audit id.
    pub(crate) async fn commit(self, result: serde_json::Value) -> Result<i64, DbErr> {
        let audit_id = write_api::insert_audit_entry(
            &self.tx,
            self.actor.name(),
            self.method,
            &self.reason,
            self.request,
            result,
        )
        .await?;
        self.tx.commit().await?;
        Ok(audit_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_support::{auth_with_key, request_with_key};
    use blockscout_service_launcher::test_database::TestDbGuard;
    use interchain_indexer_entity::{stats_assets, write_api_audit_log};
    use interchain_indexer_logic::write_api::set_stats_asset_icon_tx;
    use sea_orm::{ConnectionTrait, EntityTrait, PaginatorTrait};
    use serde_json::json;
    use std::time::Duration;

    const METHOD: &str = "SetStatsAssetIcon";
    const OLD_ICON: &str = "https://old.example/i.png";
    const NEW_ICON: &str = "https://new.example/i.png";

    /// An actor obtained the only way production code gets one: by presenting a
    /// configured key.
    fn authenticated_actor() -> Actor {
        auth_with_key("ops", "test-key")
            .authenticate(METHOD, &request_with_key((), "test-key"))
            .expect("the test key is configured")
    }

    async fn seed_asset(guard: &TestDbGuard) -> i64 {
        stats_assets::Entity::insert(stats_assets::ActiveModel {
            icon_url: sea_orm::ActiveValue::Set(Some(OLD_ICON.to_string())),
            ..Default::default()
        })
        .exec_with_returning(guard.client().as_ref())
        .await
        .unwrap()
        .id
    }

    async fn icon_of(guard: &TestDbGuard, id: i64) -> Option<String> {
        stats_assets::Entity::find_by_id(id)
            .one(guard.client().as_ref())
            .await
            .unwrap()
            .expect("asset exists")
            .icon_url
    }

    async fn audit_rows(guard: &TestDbGuard) -> u64 {
        write_api_audit_log::Entity::find()
            .count(guard.client().as_ref())
            .await
            .unwrap()
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_audit_tx_db_drop_without_commit_rolls_back_the_mutation() {
        let guard = TestDbGuard::new::<migration::Migrator>("admin_audit_tx_drop_rolls_back").await;
        let db = InterchainDatabase::new(guard.client());
        let id = seed_asset(&guard).await;
        let actor = authenticated_actor();

        let audited = AuditedTx::begin(&db, &actor, METHOD, "TICKET-1".into(), json!({}))
            .await
            .unwrap();
        let change =
            set_stats_asset_icon_tx(audited.tx(), id, Some(NEW_ICON), Duration::from_secs(5))
                .await
                .unwrap();
        assert!(change.is_some(), "the mutation ran inside the transaction");
        drop(audited);

        assert_eq!(icon_of(&guard, id).await.as_deref(), Some(OLD_ICON));
        assert_eq!(audit_rows(&guard).await, 0);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_audit_tx_db_failed_audit_insert_rolls_back_the_mutation() {
        let guard =
            TestDbGuard::new::<migration::Migrator>("admin_audit_tx_failed_audit_rolls_back").await;
        let db = InterchainDatabase::new(guard.client());
        let id = seed_asset(&guard).await;
        let actor = authenticated_actor();

        // Makes the audit insert of this method fail on its own, after the
        // mutation has already succeeded.
        guard
            .client()
            .execute_unprepared(
                "ALTER TABLE write_api_audit_log ADD CONSTRAINT test_reject_asset_icon_audit \
                 CHECK (method <> 'SetStatsAssetIcon')",
            )
            .await
            .unwrap();

        let audited = AuditedTx::begin(&db, &actor, METHOD, "TICKET-1".into(), json!({}))
            .await
            .unwrap();
        let change =
            set_stats_asset_icon_tx(audited.tx(), id, Some(NEW_ICON), Duration::from_secs(5))
                .await
                .unwrap();
        assert!(change.is_some(), "the mutation ran inside the transaction");

        audited
            .commit(json!({"icon_url_after": NEW_ICON}))
            .await
            .expect_err("the audit insert violates the CHECK, so the commit must fail");

        assert_eq!(icon_of(&guard, id).await.as_deref(), Some(OLD_ICON));
        assert_eq!(audit_rows(&guard).await, 0);
    }
}
