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
pub struct TxListResponse {
    status: String,
    message: String,
    result: Vec<serde_json::Value>,
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

pub async fn get_user_transaction_count(
    blockscout_host: &Url,
    blockscout_api_key: Option<&str>,
    address_hash: &str,
    offset: u32,
) -> Result<usize, Error> {
    let mut url = blockscout_host
        .join("/api")
        .map_err(|e| Error::BlockscoutApi(e.to_string()))?;

    let offset_str = offset.to_string();
    let mut params = vec![
        ("module", "account"),
        ("action", "txlist"),
        ("address", address_hash),
        ("page", "1"),
        ("offset", &offset_str),
        ("sort", "asc"),
    ];
    if let Some(key) = blockscout_api_key {
        params.push((API_KEY_NAME, key));
    }

    let query = serde_urlencoded::to_string(&params)
        .map_err(|e| Error::BlockscoutApi(e.to_string()))?;
    url.set_query(Some(&query));

    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::BlockscoutApi(e.to_string()))?;

    let status = response.status();
    let body = response
            .text()
            .await
            .map_err(|e| Error::BlockscoutApi(e.to_string()))?;
    
    if !status.is_success() {
        return Err(Error::BlockscoutApi(format!(
            "unexpected status {}: {}",
            status,
            body
        )));
    }

    let tx_list: TxListResponse = serde_json::from_str(&body)
        .map_err(|e| Error::BlockscoutApi(format!("parse error: {e}")))?;

    if tx_list.status != "1" {
        return Err(Error::BlockscoutApi(format!(
            "api error: {}",
            tx_list.message
        )));
    }

    Ok(tx_list.result.len())
}
