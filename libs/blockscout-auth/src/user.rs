use crate::common::{build_http_headers, extract_jwt, AuthError};
use reqwest::StatusCode;
use serde::Deserialize;
use tonic::metadata::MetadataMap;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub address_hash: String,
    pub avatar: String,
    pub email: Option<String>,
    pub name: String,
    pub nickname: String,
}

/// Извлекает JWT из метаданных, делает GET /api/account/v2/user/info?api_key=...,
/// возвращает распарсенный UserInfo.
pub async fn get_user_info_from_metadata(
    metadata: &MetadataMap,
    blockscout_host: &Url,
    blockscout_api_key: Option<&str>,
) -> Result<UserInfo, AuthError> {
    // 1) вытащить JWT
    let jwt = extract_jwt(metadata)?;

    // 2) собрать хедеры (без CSRF, метод GET безопасный)
    let headers = build_http_headers(&jwt, None)?;

    // 3) сформировать URL с опциональным api_key
    let mut url = blockscout_host
        .join("/api/account/v2/user/info")
        .expect("invalid base URL");
    if let Some(key) = blockscout_api_key {
        url.set_query(Some(&format!("api_key={}", key)));
    }

    // 4) выполнить запрос
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| AuthError::HeaderError(e.to_string()))?;

    // 5) проверить статус и десериализовать
    match resp.status() {
        StatusCode::OK => {
            let info = resp.json::<UserInfo>().await.map_err(|e| {
                AuthError::HeaderError(format!("failed to parse user info: {}", e))
            })?;
            Ok(info)
        }
        _ => Err(AuthError::HeaderError(format!(
            "user/info returned {}",
            resp.status()
        ))),
    }
}
