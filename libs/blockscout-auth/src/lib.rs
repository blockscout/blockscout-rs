use cookie::Cookie;
use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;
use tonic::{codegen::http::header::COOKIE, metadata::MetadataMap, Request};

const JWT_TOKEN_NAME: &str = "_explorer_key";
const CSRF_TOKEN_NAME: &str = "_csrf_token";

#[derive(Debug)]
pub struct AuthSuccess {
    pub user_id: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("user is unauthorized: {0}")]
    Unauthorized(String),
    #[error("blockscout invalid response: {0}")]
    BlockscoutApiError(String),
}

pub async fn auth_from_tonic<T: Serialize>(
    request: Request<T>,
    blockscout_host: &str,
) -> Result<AuthSuccess, Error> {
    let jwt = extract_jwt(request.metadata())?;
    let csrf_token = serde_json::to_value(request.into_inner())
        .map_err(|e| Error::InvalidData(format!("invalid request payload: {e}")))?
        .get(CSRF_TOKEN_NAME)
        .and_then(|token| token.as_str().map(|s| s.to_string()));
    auth_from_tokens(jwt.as_ref(), csrf_token.as_deref(), blockscout_host).await
}

pub async fn auth_from_tokens(
    jwt: &str,
    csrf_token: Option<&str>,
    _blockscout_host: &str,
) -> Result<AuthSuccess, Error> {
    // TODO: replace with actual blockscout api call
    let mut jwt = jwt.to_string();
    if let Some(cstf_token) = csrf_token {
        jwt.push_str(cstf_token)
    }
    Ok(AuthSuccess { user_id: jwt })
}

fn extract_jwt(metadata: &MetadataMap) -> Result<String, Error> {
    let cookies = get_cookies(metadata)?;
    let token = cookies
        .get(JWT_TOKEN_NAME)
        .map(|cookie| cookie.value())
        .ok_or_else(|| Error::InvalidData(format!("'{JWT_TOKEN_NAME}' not found in request")))?;
    Ok(token.to_string())
}

fn get_cookies(metadata: &MetadataMap) -> Result<HashMap<String, Cookie>, Error> {
    let cookies_raw = metadata
        .get(COOKIE.as_str())
        .map(|value| value.to_str())
        .ok_or_else(|| Error::InvalidData("no cookies were provided".to_string()))?
        .map_err(|e| Error::InvalidData(format!("invalid cookie format: {e}")))?;
    let cookies = Cookie::split_parse_encoded(cookies_raw)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::InvalidData(format!("cannot parse cookie: {e}")))?
        .into_iter()
        .map(|c| (c.name().to_string(), c))
        .collect();
    Ok(cookies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::{
        codegen::http::{header::CONTENT_TYPE, HeaderMap},
        Extensions,
    };

    fn build_headers(jwt: &str) -> HeaderMap {
        let cookies = format!(
            "intercom-id-gsgyurk3=2380c963-677d-4899-b130-01b29609f8ca; \
            intercom-session-gsgyurk3=; intercom-device-id-gsgyurk3=2fa296b4-a133-4922-b754-e3a5e446bb8e; \
            chakra-ui-color-mode=light; __cuid=0a2ad6cf04a343c0812f65aff55f0f56; \
            amp_fef1e8=3f4a1e5a-ca9c-4092-9b66-0705e0e44a21R...1gqhd6tvp.1gqhd6tvq.4.1.5; \
            adblock_detected=true; indexing_alert=false; _explorer_key={jwt}"
        );
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, cookies.parse().unwrap());
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        headers
    }

    fn build_request(jwt: &str, data: serde_json::Value) -> Request<serde_json::Value> {
        let meta = tonic::metadata::MetadataMap::from_headers(build_headers(jwt));
        Request::from_parts(meta, Extensions::default(), data)
    }

    #[test]
    fn extract_works() {
        let jwt = "VALID_JWT_TOKEN";
        let meta = tonic::metadata::MetadataMap::from_headers(build_headers(jwt));

        let token = extract_jwt(&meta).expect("failed to extract metadata");
        assert_eq!(token, jwt);
    }

    #[tokio::test]
    async fn auth_works() {
        let jwt = "VALID_JWT_TOKEN";
        let request = build_request(
            jwt,
            serde_json::json!({
                "data": "nothing"
            }),
        );
        // TODO: replace with blockscout api mock
        let success = auth_from_tonic(request, "").await.expect("failed to auth");
        assert_eq!(success.user_id, jwt);

        let jwt = "VALID_JWT_TOKEN";
        let csrf = "_PLUS_CSRF";
        let request = build_request(
            jwt,
            serde_json::json!({
                "name": "lev",
                "_csrf_token": csrf
            }),
        );
        let success = auth_from_tonic(request, "").await.expect("failed to auth");
        assert_eq!(success.user_id, format!("{jwt}{csrf}"));

        let request = Request::new(serde_json::json!({}));
        auth_from_tonic(request, "")
            .await
            .expect_err("success response for empty request");
    }
}
