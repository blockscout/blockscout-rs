mod helpers;

use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_entity::token;
use sea_orm::{ActiveValue, EntityTrait};
use tokio;

#[tokio::test]
async fn test_list_tokens_returns_all() {
    let db = crate::helpers::init_db("test", "list_tokens_returns_all").await;
    let conn = db.client();

    // Insert dummy tokens
    for i in 0..3 {
        let model = token::ActiveModel {
            id: ActiveValue::NotSet,
            zrc20_contract_address: ActiveValue::Set(format!("0x{:040x}", i)),
            asset: ActiveValue::Set(format!("0xasset{}", i)),
            foreign_chain_id: ActiveValue::Set(1),
            decimals: ActiveValue::Set(18),
            name: ActiveValue::Set(format!("Token{}", i)),
            symbol: ActiveValue::Set(format!("TK{}", i)),
            coin_type: ActiveValue::Set(zetachain_cctx_entity::sea_orm_active_enums::CoinType::Erc20),
            gas_limit: ActiveValue::Set("100000".to_string()),
            paused: ActiveValue::Set(false),
            liquidity_cap: ActiveValue::Set("1000000000".to_string()),
            icon_url: ActiveValue::Set(Some(format!("https://example.com/{}.png", i))),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        };
        token::Entity::insert(model).exec(conn.as_ref()).await.unwrap();
    }

    let database = ZetachainCctxDatabase::new(conn.clone());
    let list = database.list_tokens().await.unwrap();
    assert_eq!(list.len(), 3);
    // ensure icon_url present
    for (idx, t) in list.into_iter().enumerate() {
        assert_eq!(t.icon_url, Some(format!("https://example.com/{}.png", idx)));
    }
} 