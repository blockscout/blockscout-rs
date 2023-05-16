use serde_json::json;
use wiremock::{
    matchers::{header_regex, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

pub struct MockUser {
    pub id: i64,
    pub email: String,
    pub chain_id: i64,
    pub jwt: String,
    pub csrf_token: String,
}

pub async fn init_mocked_blockscout_auth_service(
    api_key: Option<&str>,
    users: &[MockUser],
) -> MockServer {
    let mock_server = MockServer::start().await;
    let url = "/api/account/v1/authenticate".to_string();

    for user in users {
        let mut mock = Mock::given(method("GET"))
            .and(header_regex(
                "cookie",
                &format!("_explorer_key={}", user.jwt),
            ))
            .and(path(&url));
        if let Some(api_key) = api_key {
            mock = mock.and(query_param("api_key", api_key))
        };
        mock.respond_with(respond(user)).mount(&mock_server).await;
    }

    for user in users {
        let mut mock = Mock::given(method("POST"))
            .and(header_regex(
                "cookie",
                &format!("_explorer_key={}", user.jwt),
            ))
            .and(header_regex("x-csrf-token", &user.csrf_token))
            .and(path(&url));
        if let Some(api_key) = api_key {
            mock = mock.and(query_param("api_key", api_key))
        };
        mock.respond_with(respond(user)).mount(&mock_server).await;
    }
    mock_server
}

fn respond(user: &MockUser) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "avatar": "https://lh3.googleusercontent.com/a/image",
        "email": user.email,
        "id": user.id,
        "name": "User",
        "nickname": "username",
        "uid": "google-oauth2|10238912614929394",
        "watchlist_id": user.id,
        "email_verified": true
    }))
}
