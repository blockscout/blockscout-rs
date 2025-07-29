use crate::{
    auth::Error,
    jwt_headers::{build_http_headers, extract_jwt},
};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tonic::metadata::MetadataMap;
use url::Url;

const API_KEY_NAME: &str = "api_key";

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub address_hash: Option<String>,
    pub avatar: String,
    pub email: Option<String>,
    pub name: String,
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: String,
}

pub async fn get_user_info_from_metadata(
    metadata: &MetadataMap,
    blockscout_host: &Url,
    blockscout_api_key: Option<&str>,
) -> Result<UserInfo, Error> {
    let jwt = extract_jwt(metadata)?;
    let headers = build_http_headers(&jwt, None)?;

    let mut url = blockscout_host
        .join("/api/account/v2/user/info")
        .expect("invalid base URL");
    if let Some(key) = blockscout_api_key {
        url.set_query(Some(&format!("{API_KEY_NAME}={key}")));
    }
    let client = Client::new();
    let resp = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| Error::BlockscoutApi(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| Error::BlockscoutApi(e.to_string()))?;

    match status {
        StatusCode::OK => {
            let info: UserInfo = serde_json::from_str(&body)
                .map_err(|e| Error::BlockscoutApi(format!("parse error: {e}")))?;
            Ok(info)
        }
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            let err: ErrorBody = serde_json::from_str(&body)
                .map_err(|e| Error::BlockscoutApi(format!("parse error: {e}")))?;
            if status == StatusCode::UNAUTHORIZED {
                Err(Error::Unauthorized(err.message))
            } else {
                Err(Error::Forbidden(err.message))
            }
        }
        _ => Err(Error::BlockscoutApi(format!("unexpected status {status}"))),
    }
}
