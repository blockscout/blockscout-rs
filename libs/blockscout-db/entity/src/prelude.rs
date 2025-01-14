//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.1

pub use super::{
    address_coin_balances::Entity as AddressCoinBalances,
    address_coin_balances_daily::Entity as AddressCoinBalancesDaily,
    address_contract_code_fetch_attempts::Entity as AddressContractCodeFetchAttempts,
    address_current_token_balances::Entity as AddressCurrentTokenBalances,
    address_names::Entity as AddressNames, address_tags::Entity as AddressTags,
    address_to_tags::Entity as AddressToTags,
    address_token_balances::Entity as AddressTokenBalances, addresses::Entity as Addresses,
    administrators::Entity as Administrators, beacon_blobs::Entity as BeaconBlobs,
    beacon_blobs_transactions::Entity as BeaconBlobsTransactions,
    block_rewards::Entity as BlockRewards,
    block_second_degree_relations::Entity as BlockSecondDegreeRelations, blocks::Entity as Blocks,
    bridged_tokens::Entity as BridgedTokens, constants::Entity as Constants,
    contract_methods::Entity as ContractMethods,
    contract_verification_status::Entity as ContractVerificationStatus,
    decompiled_smart_contracts::Entity as DecompiledSmartContracts,
    emission_rewards::Entity as EmissionRewards, event_notifications::Entity as EventNotifications,
    internal_transactions::Entity as InternalTransactions,
    last_fetched_counters::Entity as LastFetchedCounters, logs::Entity as Logs,
    market_history::Entity as MarketHistory, massive_blocks::Entity as MassiveBlocks,
    migrations_status::Entity as MigrationsStatus,
    missing_balance_of_tokens::Entity as MissingBalanceOfTokens,
    missing_block_ranges::Entity as MissingBlockRanges,
    pending_block_operations::Entity as PendingBlockOperations,
    proxy_implementations::Entity as ProxyImplementations,
    proxy_smart_contract_verification_statuses::Entity as ProxySmartContractVerificationStatuses,
    scam_address_badge_mappings::Entity as ScamAddressBadgeMappings,
    schema_migrations::Entity as SchemaMigrations,
    signed_authorizations::Entity as SignedAuthorizations,
    smart_contract_audit_reports::Entity as SmartContractAuditReports,
    smart_contracts::Entity as SmartContracts,
    smart_contracts_additional_sources::Entity as SmartContractsAdditionalSources,
    token_instance_metadata_refetch_attempts::Entity as TokenInstanceMetadataRefetchAttempts,
    token_instances::Entity as TokenInstances,
    token_transfer_token_id_migrator_progress::Entity as TokenTransferTokenIdMigratorProgress,
    token_transfers::Entity as TokenTransfers, tokens::Entity as Tokens,
    transaction_actions::Entity as TransactionActions,
    transaction_forks::Entity as TransactionForks, transaction_stats::Entity as TransactionStats,
    transactions::Entity as Transactions, user_contacts::Entity as UserContacts,
    user_operations::Entity as UserOperations,
    user_ops_indexer_migrations::Entity as UserOpsIndexerMigrations, users::Entity as Users,
    validators::Entity as Validators, withdrawals::Entity as Withdrawals,
};
