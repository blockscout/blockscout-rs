// SPDX-License-Identifier: LicenseRef-Blockscout

//! Mandatory regression test for the merged `m20260915_120000_add_xdai_and_cross_asset_stats`
//! migration (ADR-011). Three `ALTER TYPE bridge_type / transfer_type ADD
//! VALUE IF NOT EXISTS ...` statements at the top of that migration's SQL add
//! values to **pre-existing** enum types. PostgreSQL permits that inside a
//! transaction but forbids *using* the new value in the same transaction
//! (`ERROR: unsafe use of new value "xdai" of enum type bridge_type`), and the
//! whole migration run is one transaction (`sea-orm-migration`'s migrator
//! wraps all pending migrations in one Postgres transaction;
//! `migration::from_sql` executes each file unprepared).
//!
//! `migrate-fresh` cannot reproduce this: there, `bridge_type` /
//! `transfer_type` are created in the *same* transaction as the merged
//! migration, so PostgreSQL lifts the restriction and any accidental
//! `'xdai'` / `'erc20_to_native'` / `'native_to_erc20'` literal in the merged
//! file's later statements would pass silently. The only way to reproduce
//! the real incremental-production condition is to apply the migrations
//! that create those enum types in one committed run, then the merged
//! migration alone in a second, separate run.

#[cfg(test)]
mod tests {
    use blockscout_service_launcher::test_database::TestDbGuard;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::ConnectionTrait;
    use sea_orm_migration::{DbErr, MigrationTrait};

    /// A migrator with no migrations, used only to get [`TestDbGuard`] to
    /// create and connect to a fresh, empty database without applying any
    /// migrations itself -- the real [`Migrator`] is driven manually below in
    /// two separate calls.
    struct EmptyMigrator;

    #[async_trait::async_trait]
    impl MigratorTrait for EmptyMigrator {
        fn migrations() -> Vec<Box<dyn MigrationTrait>> {
            vec![]
        }
    }

    /// Applies migrations through `m20260910_132423_add_protocol_metadata`
    /// (the 6th entry in `Migrator::migrations()`) and commits, then applies
    /// the remaining pending migration
    /// (`m20260915_120000_add_xdai_and_cross_asset_stats`) alone in a second
    /// run. The second call must not raise "unsafe use of new value" -- if
    /// it does, some statement in the merged migration's SQL references one
    /// of the three enum literals it adds at the top of the same file,
    /// which is invisible on `migrate-fresh` and fails only here and in
    /// production.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn merged_migration_does_not_reference_its_own_new_enum_values_incrementally()
    -> Result<(), DbErr> {
        let db_guard = TestDbGuard::new::<EmptyMigrator>("migration_two_phase_enum_safety").await;
        let db = db_guard.client();

        Migrator::up(db.as_ref(), Some(6))
            .await
            .expect("phase 1 (through add_protocol_metadata) must apply cleanly");

        // Sanity: exactly one migration must still be pending -- if this
        // assumption drifts (a migration inserted or removed from the vec),
        // the test would silently stop exercising the two-phase boundary it
        // exists to guard.
        let pending = Migrator::get_pending_migrations(db.as_ref()).await?;
        assert_eq!(
            pending.len(),
            1,
            "expected exactly the merged xdai/cross-asset-stats migration to be pending after \
             phase 1; the two-phase test's `Some(6)` step count must be updated if the \
             migrations() vec changed"
        );

        Migrator::up(db.as_ref(), None).await.expect(
            "phase 2 (the merged migration alone) must not raise 'unsafe use of new value' -- \
             see this module's doc comment",
        );

        // Confirm the migration actually landed: the new enum type and the
        // cross-asset columns must exist and be usable.
        db.execute_unprepared(
            "SELECT 'mirror'::transfer_asset_linkage, 'conversion'::transfer_asset_linkage",
        )
        .await
        .expect("transfer_asset_linkage must exist with both variants after phase 2");

        db.execute_unprepared(
            "SELECT src_stats_asset_id, dst_stats_asset_id, asset_linkage FROM crosschain_transfers LIMIT 0",
        )
        .await
        .expect("crosschain_transfers must carry the new cross-asset columns after phase 2");

        db.execute_unprepared(
            "SELECT src_stats_asset_id, dst_stats_asset_id FROM stats_asset_edges LIMIT 0",
        )
        .await
        .expect("stats_asset_edges must carry the new cross-asset columns after phase 2");

        Ok(())
    }
}
