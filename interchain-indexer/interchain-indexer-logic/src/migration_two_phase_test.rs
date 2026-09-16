// SPDX-License-Identifier: LicenseRef-Blockscout

//! Regression tests for `m20260915_120000_add_xdai_and_cross_asset_stats` (ADR-011).
//! Its `ALTER TYPE bridge_type ADD VALUE 'xdai'` extends a pre-existing enum.
//! PostgreSQL permits that inside a transaction but forbids using the new value
//! before commit. `migrate-fresh` cannot reproduce this restriction: it creates
//! `bridge_type` in the same transaction. Apply the preceding migrations in a
//! committed run, then test the merged migration separately with historical data.
//! The migration also replaces the legacy `transfer_type` enum with `token_type`;
//! existing production transfers are all ERC20, regardless of unused enum variants.

#[cfg(test)]
mod tests {
    use blockscout_service_launcher::test_database::TestDbGuard;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::ConnectionTrait;
    use sea_orm_migration::{DbErr, MigrationTrait};

    /// Let the test apply the real migrations in separate committed runs.
    struct EmptyMigrator;

    #[async_trait::async_trait]
    impl MigratorTrait for EmptyMigrator {
        fn migrations() -> Vec<Box<dyn MigrationTrait>> {
            vec![]
        }
    }

    async fn seed_pre_xdai_schema(db: &sea_orm::DatabaseConnection) -> Result<(), DbErr> {
        Migrator::up(db, Some(6)).await?;
        assert_eq!(
            Migrator::get_pending_migrations(db).await?.len(),
            1,
            "expected only the merged xdai migration after phase 1; update the step count \
             if the migrations list changes"
        );
        db.execute_unprepared(
            r#"
            INSERT INTO chains (id, name) VALUES (1, 'source'), (100, 'destination');
            INSERT INTO bridges (id, name, type) VALUES (1, 'bridge', 'lockmint');
            INSERT INTO stats_assets (id, name)
              SELECT i, 'asset-' || i FROM generate_series(1, 3) AS i;
            INSERT INTO crosschain_messages (id, bridge_id, src_chain_id, dst_chain_id)
              SELECT i, 1, 1, 100 FROM generate_series(1, 2) AS i;

            -- All production transfers are ERC20. Metadata for the second
            -- token has not been fetched; migration must not invent that row.
            INSERT INTO tokens (chain_id, address, symbol)
            SELECT chain, decode(repeat(address, 20), 'hex'), label
            FROM (VALUES ('11', 'erc20')) AS fixture(address, label)
            CROSS JOIN (VALUES (1), (100)) AS chains(chain);

            -- The last mapping has neither a registry record nor a transfer.
            INSERT INTO stats_asset_tokens (stats_asset_id, chain_id, token_address)
            SELECT asset, chain, decode(repeat(address, 20), 'hex')
            FROM (VALUES (1, '11'), (2, '22'), (3, '33')) AS fixture(asset, address)
            CROSS JOIN (VALUES (1), (100)) AS chains(chain);

            INSERT INTO crosschain_transfers
              (message_id, bridge_id, type, token_src_chain_id, token_dst_chain_id,
               src_amount, dst_amount, token_src_address, token_dst_address, stats_asset_id)
            SELECT id, 1, 'erc20'::transfer_type, 1, 100, 123, 123,
                   decode(repeat(address, 20), 'hex'), decode(repeat(address, 20), 'hex'), id
            FROM (VALUES (1, '11'), (2, '22')) AS fixture(id, address);
            INSERT INTO stats_asset_edges
              (stats_asset_id, src_chain_id, dst_chain_id, transfers_count,
               cumulative_amount, decimals, amount_side, bridge_id)
            VALUES (1, 1, 100, 2, 246, 18, 'source', 1);
            "#,
        )
        .await?;
        Ok(())
    }

    async fn assert_migrated_tokens(db: &sea_orm::DatabaseConnection) -> Result<(), DbErr> {
        db.execute_unprepared(
            r#"
            DO $$ BEGIN
              ASSERT (SELECT count(*) FROM tokens) = 2,
                'migration must preserve the registry without inventing missing metadata';
              ASSERT NOT EXISTS (SELECT FROM tokens WHERE type <> 'erc20'),
                'existing token kinds must default to ERC20';
              ASSERT (SELECT count(*) FROM stats_asset_tokens) = 6;
              ASSERT NOT EXISTS (SELECT FROM stats_asset_tokens WHERE type <> 'erc20'),
                'stats kinds must default to ERC20 even without registry metadata';
              ASSERT NOT EXISTS (
                SELECT FROM information_schema.columns
                WHERE table_name = 'crosschain_transfers' AND column_name = 'type'
              ), 'legacy transfer type column must be removed';
              ASSERT NOT EXISTS (SELECT FROM pg_type WHERE typname = 'transfer_type');
              ASSERT (SELECT count(*) FROM crosschain_transfers
                WHERE src_stats_asset_id = message_id AND dst_stats_asset_id = message_id
                  AND asset_linkage = 'mirror') = 2, 'historical asset links must survive';
              ASSERT (SELECT count(*) FROM stats_asset_edges
                WHERE src_stats_asset_id = 1 AND dst_stats_asset_id = 1
                  AND transfers_count = 2 AND cumulative_amount = 246) = 1,
                'historical aggregate must survive';
            END $$;
            "#,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn merged_migration_does_not_reference_its_own_new_enum_values_incrementally()
    -> Result<(), DbErr> {
        let db_guard = TestDbGuard::new::<EmptyMigrator>("migration_two_phase_enum_safety").await;
        let db = db_guard.client();
        seed_pre_xdai_schema(db.as_ref()).await?;

        Migrator::up(db.as_ref(), None).await.expect(
            "the merged migration must not reference its new bridge enum value before commit",
        );
        assert_migrated_tokens(db.as_ref()).await?;

        // The destructive rollback preserves homogeneous endpoint kinds, and
        // restores constraint names so a subsequent upgrade works as well.
        Migrator::down(db.as_ref(), Some(1)).await?;
        db.execute_unprepared(
            r#"
            DO $$ BEGIN
              ASSERT (SELECT type = 'erc20' FROM crosschain_transfers WHERE message_id = 1),
                'rollback must restore the known homogeneous endpoint classification';
              ASSERT (SELECT type IS NULL FROM crosschain_transfers WHERE message_id = 2),
                'lossy rollback leaves the kind unknown without endpoint metadata';
              ASSERT NOT EXISTS (SELECT FROM pg_type WHERE typname = 'token_type');
            END $$;
            "#,
        )
        .await?;
        Migrator::up(db.as_ref(), None).await?;
        assert_migrated_tokens(db.as_ref()).await?;
        Ok(())
    }
}
