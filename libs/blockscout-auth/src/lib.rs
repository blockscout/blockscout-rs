use cookie::Cookie;
use std::collections::HashMap;
use thiserror::Error;
use tonic::{codegen::http::header::COOKIE, metadata::MetadataMap};

const JWT_TOKEN_NAME: &str = "_explorer_key";
const CSRF_TOKEN_NAME: &str = "_csrf_token";

#[derive(Debug)]
pub struct AuthSuccess {
    pub user_id: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("invalid jwt token: {0}")]
    InvalidJwt(String),
    #[error("invalid csrf token: {0}")]
    InvalidCsrfToken(String),
    #[error("user is unauthorized: {0}")]
    Unauthorized(String),
    #[error("blockscout invalid response: {0}")]
    BlockscoutApi(String),
}

pub async fn auth_from_metadata(
    metadata: &MetadataMap,
    is_safe_http_method: bool,
    blockscout_host: &str,
) -> Result<AuthSuccess, Error> {
    let jwt = extract_jwt(metadata)?;
    let csrf_token = if is_safe_http_method {
        None
    } else {
        Some(extract_csrf_token(metadata)?)
    };
    auth_from_tokens(&jwt, csrf_token, blockscout_host).await
}

pub async fn auth_from_tokens(
    jwt: &str,
    csrf_token: Option<&str>,
    _blockscout_host: &str,
) -> Result<AuthSuccess, Error> {
    // TODO: replace with actual blockscout api call
    let user_id = format!("{jwt}{}", csrf_token.unwrap_or_default());
    Ok(AuthSuccess { user_id })
}

fn extract_jwt(metadata: &MetadataMap) -> Result<String, Error> {
    let cookies = get_cookies(metadata)?;
    let token = cookies
        .get(JWT_TOKEN_NAME)
        .map(|cookie| cookie.value())
        .ok_or_else(|| Error::InvalidJwt(format!("'{JWT_TOKEN_NAME}' not found in request")))?;
    Ok(token.to_string())
}

fn extract_csrf_token(metadata: &MetadataMap) -> Result<&str, Error> {
    let token = metadata
        .get(CSRF_TOKEN_NAME)
        .ok_or(Error::InvalidCsrfToken("no csrf token found".to_string()))?
        .to_str()
        .map_err(|e| Error::InvalidCsrfToken(e.to_string()))?;
    Ok(token)
}

fn get_cookies(metadata: &MetadataMap) -> Result<HashMap<String, Cookie>, Error> {
    let cookies_raw = metadata
        .get(COOKIE.as_str())
        .ok_or_else(|| Error::InvalidJwt("no cookies were provided".to_string()))?
        .to_str()
        .map_err(|e| Error::InvalidJwt(format!("invalid cookie format: {e}")))?;
    let cookies = Cookie::split_parse_encoded(cookies_raw)
        .map(|val| {
            val.map(|c| (c.name().to_string(), c))
                .map_err(|e| Error::InvalidJwt(format!("cannot parse cookie: {e}")))
        })
        .collect::<Result<_, _>>()?;
    Ok(cookies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderName;
    use serde::Serialize;
    use tonic::{
        codegen::http::{header::CONTENT_TYPE, HeaderMap},
        Extensions, Request,
    };

    fn build_headers(jwt: &str, csrf_token: Option<&str>) -> HeaderMap {
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
        if let Some(csrf_token) = csrf_token {
            headers.insert(
                HeaderName::from_lowercase(b"_csrf_token").unwrap(),
                csrf_token.parse().unwrap(),
            );
        };
        headers
    }

    fn build_request<T: Serialize>(jwt: &str, csrf_token: Option<&str>, data: T) -> Request<T> {
        let meta = tonic::metadata::MetadataMap::from_headers(build_headers(jwt, csrf_token));
        Request::from_parts(meta, Extensions::default(), data)
    }

    #[test]
    fn extract_jwt_works() {
        let jwt = "VALID_JWT_TOKEN";
        let meta = tonic::metadata::MetadataMap::from_headers(build_headers(jwt, None));

        let token = extract_jwt(&meta).expect("failed to extract metadata");
        assert_eq!(token, jwt);
    }

    #[test]
    fn extract_csrf_token_works() {
        let csrf_token = "VALUD_CSRF_TOKEN";
        let meta = tonic::metadata::MetadataMap::from_headers(build_headers("", Some(csrf_token)));

        let token = extract_csrf_token(&meta).expect("failed to extract metadata");
        assert_eq!(token, csrf_token);
    }

    #[derive(Debug, Serialize)]
    struct GetBody {}

    #[derive(Debug, Serialize)]
    struct PostBody {
        name: String,
    }

    #[tokio::test]
    async fn auth_works() {
        let jwt = "VALID_JWT_TOKEN";
        let request = build_request(jwt, None, GetBody {});
        // TODO: replace with blockscout api mock
        let success = auth_from_metadata(request.metadata(), true, "")
            .await
            .expect("failed to auth");
        assert_eq!(success.user_id, jwt);

        let jwt = "VALID_JWT_TOKEN";
        let csrf_token = "_PLUS_CSRF";
        let request = build_request(
            jwt,
            Some(csrf_token),
            PostBody {
                name: "lev".to_string(),
            },
        );
        let success = auth_from_metadata(request.metadata(), false, "")
            .await
            .expect("failed to auth");
        assert_eq!(success.user_id, format!("{jwt}{csrf_token}"));

        let request = build_request(
            jwt,
            None,
            PostBody {
                name: "lev".to_string(),
            },
        );
        auth_from_metadata(request.metadata(), false, "")
            .await
            .expect_err("success response from request without csrf_token");

        let request = Request::new(GetBody {});
        auth_from_metadata(request.metadata(), true, "")
            .await
            .expect_err("success response for empty request");
    }
}
