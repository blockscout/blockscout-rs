mod helpers;

use std::sync::Arc;

use actix_phoenix_channel::ChannelCentral;

use uuid::Uuid;
use zetachain_cctx_logic::{
    channel::Channel, client::{Client, RpcSettings}, database::ZetachainCctxDatabase, indexer::Indexer, models::CoinType, settings::IndexerSettings
};

use crate::helpers::dummy_token;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_websocket_connection() {
    let db = crate::helpers::init_db("test", "websocket").await;
    let database = Arc::new(ZetachainCctxDatabase::new(db.client().clone(), 7001));

    let client = Client::new(RpcSettings::default());

    let channel: Arc<ChannelCentral<Channel>> = Arc::new(ChannelCentral::new(Channel));

    
    let _indexer = Indexer::new(
        IndexerSettings::default(),
        Arc::new(Client::new(RpcSettings::default())),
        database.clone(),
        Arc::new(channel.channel_broadcaster()),
    );

    // Start the full server with websocket support
    let (_base, channel) = crate::helpers::init_zetachain_cctx_server(
        db.db_url(),
        |mut settings| {
            settings.websocket.enabled = true; // Enable websocket
            settings.indexer.enabled = false; // Disable indexer for this test
            settings
        },
        db.client(),
        Arc::new(client),
    )
    .await;

    
    let (_client, _receiver) = channel.build_client();

    let token = dummy_token("Test Token", "TEST", None, "1", CoinType::ERC20);

    database.sync_tokens(Uuid::new_v4(), vec![token]).await.unwrap();


    let _dummy_cctx = crate::helpers::dummy_cross_chain_tx("dummy_cctx", "PendingOutbound");
    
}