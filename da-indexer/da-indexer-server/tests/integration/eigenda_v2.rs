use crate::helpers;
use blockscout_display_bytes::ToHex;
use blockscout_service_launcher::{database, database_name, test_server};
use da_indexer_logic::s3_storage_test_helpers::{
    initialize_s3_storage_and_return_settings, is_s3_storage_empty,
};
use da_indexer_proto::blockscout::da_indexer::v1 as da_indexer_v1;
use migration::Migrator;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use url::Url;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn retrieves_blobs_from_proxy_and_stores_them_in_database_db() {
    let db = database!(Migrator);
    let base_url = helpers::init_server(db.db_url()).await;
    retrieves_blobs_from_proxy_and_stores_them_in_database_template(base_url).await;
}

#[tokio::test]
async fn retrieves_blobs_from_proxy_and_stores_them_in_database_s3() {
    let db = database!(Migrator);
    let test_name = database_name!();
    let s3_storage_settings = initialize_s3_storage_and_return_settings(&test_name).await;
    let base_url = helpers::init_server_with_setup(db.db_url(), |mut settings| {
        settings.s3_storage = Some(s3_storage_settings.clone());
        settings
    })
    .await;

    retrieves_blobs_from_proxy_and_stores_them_in_database_template(base_url).await;
    assert!(!is_s3_storage_empty(&test_name).await);
}

async fn retrieves_blobs_from_proxy_and_stores_them_in_database_template(base_url: Url) {
    let commitment = "0x02f903a2e5a093139dd7d4dbe424e30430a8d820307dadc28e1fa602d1d7e557d7018e3916a383936be5f90210f901caf9018280820001f9015af842a0202102cbb63e3f4e5488ad4a4361f6e18334a803265a061ea28ba9f93f5702eaa020cbe3c25d21031e45b2bbda40ea117d7241b64739ebfddb8c0b7769e628b605f888f842a012cf2365baa7fbf040ff347a07bfea7db0b7fba823d6afc5761b406b276aa62ba0278ee6a95f9639ec6b57a74770fd790220b75ae4c500f8280131c0af6af59bf6f842a02d186f33e667d87583d64276e8842cd57aeb18e34efaf5b0bcb4091b6d657af0a00142114165bbcab6f072b98399bb7936045d4463968cb966187629d915e43dd4f887f842a013215e30f38a2b2bd2b18a81816eb4deb726236ae5e069e1908c6cf7df8370e6a02c1120e18b4fa0b050b861ebcc89a2b1230752c39e020f191e31cb48da56487cf841a01bcb5b7e649e0c7cecaee208733c3b8e84e6faa26fede0e6afaf4746b5fc5cc69fbee649368f442d83896e9cc0db7a1b0dbe61c3bc5b09e2eb789358ff652afb820400a0940f5a14effd9d8e383998dcb82d7b27d2a461478310a50636ec0d74ecaea7d2b84173fd381a677a99261d742aaa4b4e8792370daa7a8120e58af586a286d417c42d2d54cf75d3a2ac385d5bf469a3831a2a569456d958215cfee0290a07fd31a1a000c18001b8407d738adfa5cfed05c92f07e2a1feff5b3f64d4d2763e4daf4eb54e87f99c51ab87b53f1ada3e29d0bcb4facf9cf030a4c4595f5cca32268f64407bca718ecf9bf90163c0c0f888f842a02099209289cdb7e5087d0401996d2fd9b52ce5cae39c547a039f126371a7f9bca026139d9d30188c9d52468ce9dfb48c39d552243611d5b270f5497c2b8692c696f842a02b2dabbf32c0cb551d3ba9159ae5c985ebcd71d79b00fabd26a74d618065bfd6a01bef832bd3efaea9f61c0582fb123bb547546f0c5910a9dda96bcd0063d57a02f888f842a027b90b5da16ef02417ad5820223e680d2c2d19a3f1d30566cfbb7b9aa30abf6da022432d9b57d271b8dd84bfb4ccd9df36b84e422cb471b35d50d55ae83a03f16ef842a0018ed79d6c0707cc6f4ec81bcea6c4cc0096f0e3635961caf3271c3c9a36a9dfa0179360dc4646a7c49bf730e1789c00622facd7836faa3c747be0f2d824cb1412f842a00487e28c060bc61094c7a3660518655c733c9fca2ac51e4c25b2486d929f1afaa0173569e793139528921e2e1a204d1a7f022494009b65ad538496b71b7cd7bdeec20705c20805c2c0c0820001";
    let blob_data = include_bytes!(
        "../eigenda_v2_blobs/55c870c0886ce61a9b453cba2eb23bd67a6751244de4224270bb432e815824e1"
    )
    .to_vec();

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/get/{commitment}")))
        .and(query_param("commitment_mode", "standard"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("content-type", "application/octet-stream")
                .append_header("date", "Thu, 20 Nov 2025 11:03:01 GMT")
                .set_body_raw(&blob_data[..], "application/octet-stream"),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let request_path = format!(
        "/api/v1/eigenda/v2/blobs/{commitment}?proxyBaseUrl={}",
        mock_server.uri()
    );

    let initial_response: da_indexer_v1::EigenDaV2Blob =
        test_server::send_get_request(&base_url, &request_path).await;
    assert_eq!(
        da_indexer_v1::EigenDaV2Blob {
            data: blob_data.to_hex()
        },
        initial_response,
        "invalid initial response"
    );

    let second_response: da_indexer_v1::EigenDaV2Blob =
        test_server::send_get_request(&base_url, &request_path).await;
    assert_eq!(initial_response, second_response, "invalid second response");

    mock_server.verify().await;
}

#[tokio::test]
async fn returns_not_found_when_all_retrievers_fail() {
    let commitment = "0x02f90361e5a0f99fdeccf5cde94327c1fe864223496a706d7ae11e118ebe30721136f2b9ad0b838da82cf901cff901caf9018280820001f9015af842a01a68ff394a31689754270fbcc84729582a4a3a7030aecccc05cf7d531b374b30a006ac049ef38375bab35a2961e6bd5972e74a749554c95fe7c5786633e6b4dccef888f842a02d3b94cd6a78f76878f7ad0acb67008311c47e7ee26f8f2573c58e481d12e501a00bfcb198b699ec82a2004a1ba3e54448fe6bc49086f76e2818bfea739427fc51f842a01e9ff4fbc3b1e8aa80976fa32df77d06fe1a9f0b12d81525f01b979dca0fe518a02b2c934aa8dc50959895e5d38070676d3d299b823ad75b60c1634cf2097e846ff888f842a003a8066333c7b87d86caffd67c0e960beb844194f29e1905dddea0ac56e5d946a0070ddf01ccccfe1cf5bab058f51f0c94a8a688b0590611566f76f201360c1daff842a026f4e3bd414b633bc820a902674cd726e5d76600fab532346c1d31be8f87f3eea02fe5735eb6170f3b1784e9563c4a20e009e55ff936f7145e6dd03991fa004c638180a0a6c313e94b27c4a4ecdd5e9e732c6d4c1dddc38b5e9f37631a6aad4787fc42afb8414784589cf7067416150e1954abb21b73313d012859bea882eb30d24eb51d8cd3557cca3a98b8320c60a1f8a7172a882995b4c9cb8a1f3e3f9aa7f1addc50c43200c1808080f90163c0c0f888f842a02099209289cdb7e5087d0401996d2fd9b52ce5cae39c547a039f126371a7f9bca026139d9d30188c9d52468ce9dfb48c39d552243611d5b270f5497c2b8692c696f842a02b2dabbf32c0cb551d3ba9159ae5c985ebcd71d79b00fabd26a74d618065bfd6a01bef832bd3efaea9f61c0582fb123bb547546f0c5910a9dda96bcd0063d57a02f888f842a027b90b5da16ef02417ad5820223e680d2c2d19a3f1d30566cfbb7b9aa30abf6da022432d9b57d271b8dd84bfb4ccd9df36b84e422cb471b35d50d55ae83a03f16ef842a0018ed79d6c0707cc6f4ec81bcea6c4cc0096f0e3635961caf3271c3c9a36a9dfa0179360dc4646a7c49bf730e1789c00622facd7836faa3c747be0f2d824cb1412f842a01c8d0bcdc47b53535096f62e486a74609a5f1d9a03344936236186721df57829a00fc54c6f0db8ab0c0924178cf5cd75aa86d250bb1f8f14d63ea790b23da1bd28c20705c20805c2c0c0820001";

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/get/{commitment}")))
        .and(query_param("commitment_mode", "standard"))
        // for some reason the proxy responds with 500 in case the blob cannot be found
        .respond_with(ResponseTemplate::new(500)
            .set_body_string("get request failed with serializedCert (version 2) f90361e5a0f99fdeccf5cde94327c1fe864223496a706d7ae11e118ebe30721136f2b9ad0b838da82cf901cff901caf9018280820001f9015af842a01a68ff394a31689754270fbcc84729582a4a3a7030aecccc05cf7d531b374b30a006ac049ef38375bab35a2961e6bd5972e74a749554c95fe7c5786633e6b4dccef888f842a02d3b94cd6a78f76878f7ad0acb67008311c47e7ee26f8f2573c58e481d12e501a00bfcb198b699ec82a2004a1ba3e54448fe6bc49086f76e2818bfea739427fc51f842a01e9ff4fbc3b1e8aa80976fa32df77d06fe1a9f0b12d81525f01b979dca0fe518a02b2c934aa8dc50959895e5d38070676d3d299b823ad75b60c1634cf2097e846ff888f842a003a8066333c7b87d86caffd67c0e960beb844194f29e1905dddea0ac56e5d946a0070ddf01ccccfe1cf5bab058f51f0c94a8a688b0590611566f76f201360c1daff842a026f4e3bd414b633bc820a902674cd726e5d76600fab532346c1d31be8f87f3eea02fe5735eb6170f3b1784e9563c4a20e009e55ff936f7145e6dd03991fa004c638180a0a6c313e94b27c4a4ecdd5e9e732c6d4c1dddc38b5e9f37631a6aad4787fc42afb8414784589cf7067416150e1954abb21b73313d012859bea882eb30d24eb51d8cd3557cca3a98b8320c60a1f8a7172a882995b4c9cb8a1f3e3f9aa7f1addc50c43200c1808080f90163c0c0f888f842a02099209289cdb7e5087d0401996d2fd9b52ce5cae39c547a039f126371a7f9bca026139d9d30188c9d52468ce9dfb48c39d552243611d5b270f5497c2b8692c696f842a02b2dabbf32c0cb551d3ba9159ae5c985ebcd71d79b00fabd26a74d618065bfd6a01bef832bd3efaea9f61c0582fb123bb547546f0c5910a9dda96bcd0063d57a02f888f842a027b90b5da16ef02417ad5820223e680d2c2d19a3f1d30566cfbb7b9aa30abf6da022432d9b57d271b8dd84bfb4ccd9df36b84e422cb471b35d50d55ae83a03f16ef842a0018ed79d6c0707cc6f4ec81bcea6c4cc0096f0e3635961caf3271c3c9a36a9dfa0179360dc4646a7c49bf730e1789c00622facd7836faa3c747be0f2d824cb1412f842a01c8d0bcdc47b53535096f62e486a74609a5f1d9a03344936236186721df57829a00fc54c6f0db8ab0c0924178cf5cd75aa86d250bb1f8f14d63ea790b23da1bd28c20705c20805c2c0c0820001: get data from V2 backend: all retrievers failed: %!w(<nil>)\n")
        )
        .mount(&mock_server)
        .await;

    let db = database!(Migrator);
    let base = helpers::init_server(db.db_url()).await;

    let request_path = format!(
        "/api/v1/eigenda/v2/blobs/{commitment}?proxyBaseUrl={}",
        mock_server.uri()
    );

    let response = reqwest::get(base.join(&request_path).unwrap())
        .await
        .expect("error sending request");

    assert_eq!(
        reqwest::StatusCode::NOT_FOUND,
        response.status(),
        "invalid response status code"
    );
}

#[tokio::test]
async fn proxy_errors_are_propagated_as_is() {
    let commitment = "0xf903a2e5a093139dd7d4dbe424e30430a8d820307dadc28e1fa602d1d7e557d7018e3916a383936be5f90210f901caf9018280820001f9015af842a0202102cbb63e3f4e5488ad4a4361f6e18334a803265a061ea28ba9f93f5702eaa020cbe3c25d21031e45b2bbda40ea117d7241b64739ebfddb8c0b7769e628b605f888f842a012cf2365baa7fbf040ff347a07bfea7db0b7fba823d6afc5761b406b276aa62ba0278ee6a95f9639ec6b57a74770fd790220b75ae4c500f8280131c0af6af59bf6f842a02d186f33e667d87583d64276e8842cd57aeb18e34efaf5b0bcb4091b6d657af0a00142114165bbcab6f072b98399bb7936045d4463968cb966187629d915e43dd4f887f842a013215e30f38a2b2bd2b18a81816eb4deb726236ae5e069e1908c6cf7df8370e6a02c1120e18b4fa0b050b861ebcc89a2b1230752c39e020f191e31cb48da56487cf841a01bcb5b7e649e0c7cecaee208733c3b8e84e6faa26fede0e6afaf4746b5fc5cc69fbee649368f442d83896e9cc0db7a1b0dbe61c3bc5b09e2eb789358ff652afb820400a0940f5a14effd9d8e383998dcb82d7b27d2a461478310a50636ec0d74ecaea7d2b84173fd381a677a99261d742aaa4b4e8792370daa7a8120e58af586a286d417c42d2d54cf75d3a2ac385d5bf469a3831a2a569456d958215cfee0290a07fd31a1a000c18001b8407d738adfa5cfed05c92f07e2a1feff5b3f64d4d2763e4daf4eb54e87f99c51ab87b53f1ada3e29d0bcb4facf9cf030a4c4595f5cca32268f64407bca718ecf9bf90163c0c0f888f842a02099209289cdb7e5087d0401996d2fd9b52ce5cae39c547a039f126371a7f9bca026139d9d30188c9d52468ce9dfb48c39d552243611d5b270f5497c2b8692c696f842a02b2dabbf32c0cb551d3ba9159ae5c985ebcd71d79b00fabd26a74d618065bfd6a01bef832bd3efaea9f61c0582fb123bb547546f0c5910a9dda96bcd0063d57a02f888f842a027b90b5da16ef02417ad5820223e680d2c2d19a3f1d30566cfbb7b9aa30abf6da022432d9b57d271b8dd84bfb4ccd9df36b84e422cb471b35d50d55ae83a03f16ef842a0018ed79d6c0707cc6f4ec81bcea6c4cc0096f0e3635961caf3271c3c9a36a9dfa0179360dc4646a7c49bf730e1789c00622facd7836faa3c747be0f2d824cb1412f842a00487e28c060bc61094c7a3660518655c733c9fca2ac51e4c25b2486d929f1afaa0173569e793139528921e2e1a204d1a7f022494009b65ad538496b71b7cd7bdeec20705c20805c2c0c0820001";

    let error_body = "unsupported version byte f9: unknown EigenDA cert version: 249\nparsing error: parsing version byte: unsupported version byte f9: unknown EigenDA cert version: 249\n";
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/get/{commitment}")))
        .and(query_param("commitment_mode", "standard"))
        .respond_with(ResponseTemplate::new(400).set_body_string(error_body))
        .mount(&mock_server)
        .await;

    let db = database!(Migrator);
    let base = helpers::init_server(db.db_url()).await;

    let request_path = format!(
        "/api/v1/eigenda/v2/blobs/{commitment}?proxyBaseUrl={}",
        mock_server.uri()
    );

    let response = reqwest::get(base.join(&request_path).unwrap())
        .await
        .expect("error sending request");

    assert_eq!(
        reqwest::StatusCode::BAD_REQUEST,
        response.status(),
        "invalid response status code"
    );

    #[derive(Deserialize)]
    struct Response {
        message: String,
    }
    let body: Response = response.json().await.unwrap();
    assert_eq!(error_body, body.message, "invalid response body");
}

#[tokio::test]
async fn proxy_base_url_without_schema_returns_bad_request() {
    let commitment = "0x02f903a2e5a093139dd7d4dbe424e30430a8d820307dadc28e1fa602d1d7e557d7018e3916a383936be5f90210f901caf9018280820001f9015af842a0202102cbb63e3f4e5488ad4a4361f6e18334a803265a061ea28ba9f93f5702eaa020cbe3c25d21031e45b2bbda40ea117d7241b64739ebfddb8c0b7769e628b605f888f842a012cf2365baa7fbf040ff347a07bfea7db0b7fba823d6afc5761b406b276aa62ba0278ee6a95f9639ec6b57a74770fd790220b75ae4c500f8280131c0af6af59bf6f842a02d186f33e667d87583d64276e8842cd57aeb18e34efaf5b0bcb4091b6d657af0a00142114165bbcab6f072b98399bb7936045d4463968cb966187629d915e43dd4f887f842a013215e30f38a2b2bd2b18a81816eb4deb726236ae5e069e1908c6cf7df8370e6a02c1120e18b4fa0b050b861ebcc89a2b1230752c39e020f191e31cb48da56487cf841a01bcb5b7e649e0c7cecaee208733c3b8e84e6faa26fede0e6afaf4746b5fc5cc69fbee649368f442d83896e9cc0db7a1b0dbe61c3bc5b09e2eb789358ff652afb820400a0940f5a14effd9d8e383998dcb82d7b27d2a461478310a50636ec0d74ecaea7d2b84173fd381a677a99261d742aaa4b4e8792370daa7a8120e58af586a286d417c42d2d54cf75d3a2ac385d5bf469a3831a2a569456d958215cfee0290a07fd31a1a000c18001b8407d738adfa5cfed05c92f07e2a1feff5b3f64d4d2763e4daf4eb54e87f99c51ab87b53f1ada3e29d0bcb4facf9cf030a4c4595f5cca32268f64407bca718ecf9bf90163c0c0f888f842a02099209289cdb7e5087d0401996d2fd9b52ce5cae39c547a039f126371a7f9bca026139d9d30188c9d52468ce9dfb48c39d552243611d5b270f5497c2b8692c696f842a02b2dabbf32c0cb551d3ba9159ae5c985ebcd71d79b00fabd26a74d618065bfd6a01bef832bd3efaea9f61c0582fb123bb547546f0c5910a9dda96bcd0063d57a02f888f842a027b90b5da16ef02417ad5820223e680d2c2d19a3f1d30566cfbb7b9aa30abf6da022432d9b57d271b8dd84bfb4ccd9df36b84e422cb471b35d50d55ae83a03f16ef842a0018ed79d6c0707cc6f4ec81bcea6c4cc0096f0e3635961caf3271c3c9a36a9dfa0179360dc4646a7c49bf730e1789c00622facd7836faa3c747be0f2d824cb1412f842a00487e28c060bc61094c7a3660518655c733c9fca2ac51e4c25b2486d929f1afaa0173569e793139528921e2e1a204d1a7f022494009b65ad538496b71b7cd7bdeec20705c20805c2c0c0820001";

    let db = database!(Migrator);
    let base = helpers::init_server(db.db_url()).await;

    let invalid_proxy_base_url = "eigenda-proxy.com:3100";
    let request_path =
        format!("/api/v1/eigenda/v2/blobs/{commitment}?proxyBaseUrl={invalid_proxy_base_url}");

    let response = reqwest::get(base.join(&request_path).unwrap())
        .await
        .expect("error sending request");

    assert_eq!(
        reqwest::StatusCode::BAD_REQUEST,
        response.status(),
        "invalid response status code"
    );
}
