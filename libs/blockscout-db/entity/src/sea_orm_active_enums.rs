//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.8

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(
    rs_type = "String",
    db_type = "Enum",
    enum_name = "entry_point_version"
)]
pub enum EntryPointVersion {
    #[sea_orm(string_value = "v0.6")]
    V06,
    #[sea_orm(string_value = "v0.7")]
    V07,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "proxy_type")]
pub enum ProxyType {
    #[sea_orm(string_value = "eip1167")]
    Eip1167,
    #[sea_orm(string_value = "eip1967")]
    Eip1967,
    #[sea_orm(string_value = "eip1822")]
    Eip1822,
    #[sea_orm(string_value = "eip930")]
    Eip930,
    #[sea_orm(string_value = "master_copy")]
    MasterCopy,
    #[sea_orm(string_value = "basic_implementation")]
    BasicImplementation,
    #[sea_orm(string_value = "basic_get_implementation")]
    BasicGetImplementation,
    #[sea_orm(string_value = "comptroller")]
    Comptroller,
    #[sea_orm(string_value = "eip2535")]
    Eip2535,
    #[sea_orm(string_value = "clone_with_immutable_arguments")]
    CloneWithImmutableArguments,
    #[sea_orm(string_value = "eip7702")]
    Eip7702,
    #[sea_orm(string_value = "unknown")]
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "sponsor_type")]
pub enum SponsorType {
    #[sea_orm(string_value = "wallet_deposit")]
    WalletDeposit,
    #[sea_orm(string_value = "wallet_balance")]
    WalletBalance,
    #[sea_orm(string_value = "paymaster_sponsor")]
    PaymasterSponsor,
    #[sea_orm(string_value = "paymaster_hybrid")]
    PaymasterHybrid,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(
    rs_type = "String",
    db_type = "Enum",
    enum_name = "transaction_actions_protocol"
)]
pub enum TransactionActionsProtocol {
    #[sea_orm(string_value = "uniswap_v3")]
    UniswapV3,
    #[sea_orm(string_value = "opensea_v1_1")]
    OpenseaV11,
    #[sea_orm(string_value = "wrapping")]
    Wrapping,
    #[sea_orm(string_value = "approval")]
    Approval,
    #[sea_orm(string_value = "zkbob")]
    Zkbob,
    #[sea_orm(string_value = "aave_v3")]
    AaveV3,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(
    rs_type = "String",
    db_type = "Enum",
    enum_name = "transaction_actions_type"
)]
pub enum TransactionActionsType {
    #[sea_orm(string_value = "mint_nft")]
    MintNft,
    #[sea_orm(string_value = "mint")]
    Mint,
    #[sea_orm(string_value = "burn")]
    Burn,
    #[sea_orm(string_value = "collect")]
    Collect,
    #[sea_orm(string_value = "swap")]
    Swap,
    #[sea_orm(string_value = "sale")]
    Sale,
    #[sea_orm(string_value = "cancel")]
    Cancel,
    #[sea_orm(string_value = "transfer")]
    Transfer,
    #[sea_orm(string_value = "wrap")]
    Wrap,
    #[sea_orm(string_value = "unwrap")]
    Unwrap,
    #[sea_orm(string_value = "approve")]
    Approve,
    #[sea_orm(string_value = "revoke")]
    Revoke,
    #[sea_orm(string_value = "withdraw")]
    Withdraw,
    #[sea_orm(string_value = "deposit")]
    Deposit,
    #[sea_orm(string_value = "borrow")]
    Borrow,
    #[sea_orm(string_value = "supply")]
    Supply,
    #[sea_orm(string_value = "repay")]
    Repay,
    #[sea_orm(string_value = "flash_loan")]
    FlashLoan,
    #[sea_orm(string_value = "enable_collateral")]
    EnableCollateral,
    #[sea_orm(string_value = "disable_collateral")]
    DisableCollateral,
    #[sea_orm(string_value = "liquidation_call")]
    LiquidationCall,
}
