// SPDX-License-Identifier: LicenseRef-Blockscout

//! Regression tests for `m20260915_120000_add_xdai_and_cross_asset_stats` (ADR-011).
//! Its `ALTER TYPE bridge_type ADD VALUE 'xdai'` extends a pre-existing enum.
//! PostgreSQL permits that inside a transaction but forbids using the new value
//! before commit. `migrate-fresh` cannot reproduce this restriction: it creates
//! `bridge_type` in the same transaction. Apply the preceding migrations in a
//! committed run, then test the merged migration separately with historical data.
//! The migration also replaces the legacy `transfer_type` enum with `token_type`;
//! those classifications must survive on both registry and stats token records.

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
              SELECT i, 'asset-' || i FROM generate_series(1, 6) AS i;
            INSERT INTO crosschain_messages (id, bridge_id, src_chain_id, dst_chain_id)
              SELECT i, 1, 1, 100 FROM generate_series(1, 5) AS i;

            -- ERC1155 metadata is absent: migration must reconstruct its kind
            -- from transfers. The NULL legacy classification defaults to ERC20.
            INSERT INTO tokens (chain_id, address, symbol)
            SELECT chain, decode(repeat(address, 20), 'hex'), label
            FROM (VALUES ('11', 'erc20'), ('22', 'unknown'),
                         ('00', 'native'), ('33', 'erc721')) AS fixture(address, label)
            CROSS JOIN (VALUES (1), (100)) AS chains(chain);

            -- The last mapping has neither a registry record nor a transfer.
            INSERT INTO stats_asset_tokens (stats_asset_id, chain_id, token_address)
            SELECT asset, chain, decode(repeat(address, 20), 'hex')
            FROM (VALUES (1, '11'), (2, '22'), (3, '00'),
                         (4, '33'), (5, '44'), (6, '55')) AS fixture(asset, address)
            CROSS JOIN (VALUES (1), (100)) AS chains(chain);

            INSERT INTO crosschain_transfers
              (message_id, bridge_id, type, token_src_chain_id, token_dst_chain_id,
               src_amount, dst_amount, token_src_address, token_dst_address, stats_asset_id)
            SELECT id, 1, kind::transfer_type, 1, 100, 123, 123,
                   decode(repeat(address, 20), 'hex'), decode(repeat(address, 20), 'hex'), id
            FROM (VALUES (1, 'erc20', '11'), (2, NULL, '22'), (3, 'native', '00'),
                         (4, 'erc721', '33'), (5, 'erc1155', '44')) AS fixture(id, kind, address);
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
              ASSERT (SELECT count(*) FROM tokens) = 10,
                'missing NFT registry records must be inserted';
              ASSERT NOT EXISTS (
                SELECT FROM tokens t
                JOIN (VALUES ('11', 'erc20'), ('22', 'erc20'), ('00', 'native'),
                             ('33', 'erc721'), ('44', 'erc1155')) AS expected(address, kind)
                  ON t.address = decode(repeat(expected.address, 20), 'hex')
                WHERE t.type::text <> expected.kind
              ), 'token classifications must survive migration';
              ASSERT (SELECT count(*) FROM stats_asset_tokens) = 12;
              ASSERT NOT EXISTS (
                SELECT FROM stats_asset_tokens t
                JOIN (VALUES ('11', 'erc20'), ('22', 'erc20'), ('00', 'native'),
                             ('33', 'erc721'), ('44', 'erc1155'), ('55', 'erc20'))
                  AS expected(address, kind)
                  ON t.token_address = decode(repeat(expected.address, 20), 'hex')
                WHERE t.type::text <> expected.kind
              ), 'stats mappings must receive the token kind, including missing registry metadata';
              ASSERT NOT EXISTS (
                SELECT FROM information_schema.columns
                WHERE table_name = 'crosschain_transfers' AND column_name = 'type'
              ), 'legacy transfer type column must be removed';
              ASSERT NOT EXISTS (SELECT FROM pg_type WHERE typname = 'transfer_type');
              ASSERT (SELECT count(*) FROM crosschain_transfers
                WHERE src_stats_asset_id = message_id AND dst_stats_asset_id = message_id
                  AND asset_linkage = 'mirror') = 5, 'historical asset links must survive';
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
              ASSERT (SELECT count(*) FROM crosschain_transfers
                WHERE type::text = CASE message_id
                  WHEN 1 THEN 'erc20' WHEN 2 THEN 'erc20' WHEN 3 THEN 'native'
                  WHEN 4 THEN 'erc721' WHEN 5 THEN 'erc1155' END) = 5,
                'rollback must restore homogeneous endpoint classifications';
              ASSERT NOT EXISTS (SELECT FROM pg_type WHERE typname = 'token_type');
            END $$;
            "#,
        )
        .await?;
        Migrator::up(db.as_ref(), None).await?;
        assert_migrated_tokens(db.as_ref()).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_merged_migration_conflicting_legacy_nft_kinds_rolls_back() -> Result<(), DbErr> {
        let db_guard = TestDbGuard::new::<EmptyMigrator>("migration_conflicting_token_kinds").await;
        let db = db_guard.client();
        seed_pre_xdai_schema(db.as_ref()).await?;
        db.execute_unprepared(
            r#"
            INSERT INTO crosschain_transfers
              (message_id, bridge_id, index, type, token_src_chain_id, token_dst_chain_id,
               src_amount, dst_amount, token_src_address, token_dst_address)
            VALUES (4, 1, 1, 'erc1155', 1, 100, 1, 1,
                    decode(repeat('33', 20), 'hex'), decode(repeat('33', 20), 'hex'));
            "#,
        )
        .await?;
        let error = Migrator::up(db.as_ref(), None)
            .await
            .expect_err("one token cannot silently choose between contradictory NFT kinds");
        assert!(error.to_string().contains("legacy_token_kinds"), "{error}");
        db.execute_unprepared(
            r#"
            DO $$ BEGIN
              ASSERT NOT EXISTS (SELECT FROM pg_type WHERE typname = 'token_type');
              ASSERT (SELECT count(*) FROM crosschain_transfers WHERE type = 'erc721') = 1;
              ASSERT (SELECT count(*) FROM crosschain_transfers WHERE type = 'erc1155') = 2;
            END $$;
            "#,
        )
        .await?;
        assert_eq!(
            Migrator::get_pending_migrations(db.as_ref()).await?.len(),
            1
        );
        Ok(())
    }
}
