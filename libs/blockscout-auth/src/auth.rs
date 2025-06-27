
use crate::common::{build_http_headers, extract_csrf_token, extract_jwt, CommonError};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use thiserror::Error;
use tonic::metadata::MetadataMap;
use url::Url;

const API_KEY_NAME: &str = "api_key";

#[derive(Debug, Clone, Deserialize)]
pub struct AuthSuccess {
    pub avatar: String,
    pub email: String,
    pub id: i64,
    pub name: String,
    pub nickname: String,
    pub uid: String,
    pub watchlist_id: i64,
    pub email_verified: bool,
}
#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub address_hash: String,
    pub avatar: String,
    pub email: Option<String>,
    pub name: String,
    pub nickname: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AuthFailed {
    pub message: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Common(#[from] CommonError),
    #[error("blockscout API error: {0}")]
    BlockscoutApi(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
}

impl From<CommonError> for Error {
    fn from(e: CommonError) -> Self {
        Error::Common(e)
    }
}

pub async fn auth_from_metadata(
    metadata: &MetadataMap,
    is_safe_http_method: bool,
    blockscout_host: &Url,
    blockscout_api_key: Option<&str>,
) -> Result<AuthSuccess, Error> {
    let jwt = extract_jwt(metadata)?;
    let csrf_opt = if is_safe_http_method {
        None
    } else {
        Some(extract_csrf_token(metadata)?)
    };
    auth_from_tokens(&jwt, csrf_opt.as_deref(), blockscout_host, blockscout_api_key).await
}

pub async fn auth_from_tokens(
    jwt: &str,
    csrf_token: Option<&str>,
    blockscout_host: &Url,
    blockscout_api_key: Option<&str>,
) -> Result<AuthSuccess, Error> {
    let mut url = blockscout_host
        .join("/api/account/v2/authenticate")
        .expect("invalid base url");
    url.set_query(
        blockscout_api_key
            .map(|k| format!("{API_KEY_NAME}={k}"))
            .as_deref(),
    );

    let headers = build_http_headers(jwt, csrf_token)?;
    let client = Client::new();
    let resp = if csrf_token.is_some() {
        client.post(url)
    } else {
        client.get(url)
    }
    .headers(headers)
    .send()
    .await
    .map_err(|e| Error::BlockscoutApi(e.to_string()))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| Error::BlockscoutApi(e.to_string()))?;

    match status {
        StatusCode::OK => {
            let success: AuthSuccess =
                serde_json::from_str(&body).map_err(|e| Error::BlockscoutApi(e.to_string()))?;
            Ok(success)
        }
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            let failed: AuthFailed =
                serde_json::from_str(&body).map_err(|e| Error::BlockscoutApi(e.to_string()))?;
            if status == StatusCode::UNAUTHORIZED {
                Err(Error::Unauthorized(failed.message))
            } else {
                Err(Error::Forbidden(failed.message))
            }
        }
        _ => Err(Error::BlockscoutApi(format!("unexpected status {status}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init_mocked_blockscout_auth_service, MockUser};
    use reqwest::header::{HeaderMap, HeaderName};
    use serde::Serialize;
    use tonic::{codegen::http::header::CONTENT_TYPE, Extensions, Request};
    use url::Url;

    fn build_headers(jwt: &str, csrf: Option<&str>, in_cookie: bool) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if in_cookie {
            let c = format!("_explorer_key={jwt}");
            headers.insert("cookie", c.parse().unwrap());
        } else {
            headers.insert("authorization", jwt.parse().unwrap());
        }
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        if let Some(csrf) = csrf {
            headers.insert(
                HeaderName::from_lowercase(b"x-csrf-token").unwrap(),
                csrf.parse().unwrap(),
            );
        }
        headers
    }

    fn build_request<T: Serialize>(jwt: &str, csrf: Option<&str>, data: T) -> Request<T> {
        let meta = MetadataMap::from_headers(build_headers(jwt, csrf, false));
        Request::from_parts(meta, Extensions::default(), data)
    }

    #[test]
    fn extract_jwt_both() {
        let jwt = "TOKEN";
        let meta = MetadataMap::from_headers(build_headers(jwt, None, true));
        assert_eq!(extract_jwt(&meta).unwrap(), jwt);
        let meta = MetadataMap::from_headers(build_headers(jwt, None, false));
        assert_eq!(extract_jwt(&meta).unwrap(), jwt);
    }

    #[test]
    fn extract_csrf() {
        let csrf = "CSRF";
        let meta = MetadataMap::from_headers(build_headers("", Some(csrf), true));
        assert_eq!(extract_csrf_token(&meta).unwrap(), csrf);
    }

    #[derive(Serialize)]
    struct G;

    #[derive(Serialize)]
    struct P { name: String }

    #[tokio::test]
    async fn full_auth_flow() {
        let users = [MockUser {
            id: 1,
            email: "a@b".into(),
            chain_id: 1,
            jwt: "jwt1".into(),
            csrf_token: "csrf1".into(),
        }];
        let api_key = Some("KEY");
        let mock = init_mocked_blockscout_auth_service(api_key, &users).await;
        let host = Url::parse(&mock.uri()).unwrap();

        // GET
        let req = build_request("jwt1", None, G);
        let ok = auth_from_metadata(req.metadata(), true, &host, api_key).await.unwrap();
        assert_eq!(ok.id, 1);

        // POST
        let req = build_request("jwt1", Some("csrf1"), P { name: "x".into() });
        let ok = auth_from_metadata(req.metadata(), false, &host, api_key).await.unwrap();
        assert_eq!(ok.id, 1);
        
        // POST без CSRF — ошибка
        let req = build_request("jwt1", None, P { name: "x".into() });
        auth_from_metadata(req.metadata(), false, &host, api_key)
            .await
            .expect_err("should fail");

        let req = Request::new(G);
        auth_from_metadata(req.metadata(), true, &host, api_key)
            .await
            .expect_err("should fail");
    }

    #[tokio::test]
    async fn auth_no_api_key() {
        let users = [MockUser {
            id: 2,
            email: "c@d".into(),
            chain_id: 1,
            jwt: "jwt2".into(),
            csrf_token: "csrf2".into(),
        }];
        let mock = init_mocked_blockscout_auth_service(None, &users).await;
        let host = Url::parse(&mock.uri()).unwrap();
        let req = build_request("jwt2", None, G);
        let ok = auth_from_metadata(req.metadata(), true, &host, None).await.unwrap();
        assert_eq!(ok.id, 2);
    }
}
