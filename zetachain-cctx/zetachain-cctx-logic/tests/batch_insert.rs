use blockscout_service_launcher::test_database::TestDbGuard;
use migration::sea_orm::TransactionTrait;
use uuid::Uuid;
use zetachain_cctx_entity::token as TokenEntity;
use zetachain_cctx_logic::database::ZetachainCctxDatabase;
mod helpers;

#[tokio::test]
async fn test_batch_insert() {
    use migration::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use zetachain_cctx_entity::{cross_chain_tx, outbound_params};

    let db = TestDbGuard::new::<migration::Migrator>("batch_insert").await;

    let tx = db.client().begin().await.unwrap();

    let token = zetachain_cctx_logic::models::Token {
        name: "dummy_token_1".to_string(),
        symbol: "DUMMY".to_string(),
        asset: "0x0000000000000000000000000000000000000001".to_string(),
        foreign_chain_id: "7001".to_string(),
        coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };

    let mut cctx = helpers::dummy_cross_chain_tx("bad_cctx_index", "OutboundMined");

    cctx.inbound_params.asset = token.asset.clone();
    cctx.inbound_params.coin_type = token.coin_type.clone();
    cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();

    let cctxs = vec![cctx];

    let job_id = Uuid::new_v4();

    let database = ZetachainCctxDatabase::new(db.client(), 7001);

    database.setup_db().await.unwrap();

    let res = database
        .batch_insert_transactions(job_id, &cctxs, &tx, None)
        .await;

    tx.commit().await.unwrap();
    assert!(res.is_ok());

    let cctx = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq("bad_cctx_index"))
        .one(db.client().as_ref())
        .await
        .unwrap();

    assert!(cctx.is_some());
    let cctx = cctx.unwrap();
    let outbound_params = outbound_params::Entity::find()
        .filter(outbound_params::Column::CrossChainTxId.eq(cctx.id))
        .all(db.client().as_ref())
        .await
        .unwrap();

    assert_eq!(outbound_params.len(), 2);

    assert_eq!(outbound_params.first().unwrap().receiver_chain_id, 2);

    assert_eq!(outbound_params.last().unwrap().receiver_chain_id, 3);

    let unknown_token: TokenEntity::Model = TokenEntity::Entity::find()
        .filter(TokenEntity::Column::Zrc20ContractAddress.eq("UNKNOWN"))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(cctx.token_id, Some(unknown_token.id));
}
