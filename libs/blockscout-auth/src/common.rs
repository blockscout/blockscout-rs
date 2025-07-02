use cookie::Cookie;
use reqwest::header::{HeaderMap, HeaderValue};
use thiserror::Error;
use tonic::metadata::MetadataMap;

pub const HEADER_JWT_TOKEN_NAME: &str = "authorization";
pub const COOKIE_JWT_TOKEN_NAME: &str = "_explorer_key";
pub const CSRF_TOKEN_NAME: &str = "x-csrf-token";

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("invalid jwt token: {0}")]
    InvalidJwt(String),
    #[error("invalid csrf token: {0}")]
    InvalidCsrfToken(String),
    #[error("cannot build headers: {0}")]
    HeaderError(String),
}

pub type CommonError = AuthError;

pub fn extract_jwt(metadata: &MetadataMap) -> Result<String, AuthError> {
    let cookies = get_cookies(metadata)?;
    let cookie_jwt = cookies
        .get(COOKIE_JWT_TOKEN_NAME)
        .map(|c| c.value().to_string());
    let header_jwt = metadata
        .get(HEADER_JWT_TOKEN_NAME)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    cookie_jwt
        .or(header_jwt)
        .ok_or_else(|| AuthError::InvalidJwt("jwt not found".into()))
}

pub fn extract_csrf_token(metadata: &MetadataMap) -> Result<String, AuthError> {
    metadata
        .get(CSRF_TOKEN_NAME)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| AuthError::InvalidCsrfToken("csrf not found".into()))
}

fn get_cookies(
    metadata: &MetadataMap,
) -> Result<std::collections::HashMap<String, Cookie>, AuthError> {
    let raw = metadata
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    Cookie::split_parse_encoded(raw.to_string())
        .map(|r| r.map(|c| (c.name().to_string(), c)))
        .collect::<Result<_, _>>()
        .map_err(|e| AuthError::InvalidJwt(format!("cannot parse cookie: {e}")))
}

pub fn build_http_headers(jwt: &str, csrf_token: Option<&str>) -> Result<HeaderMap, AuthError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_str(&format!("{}={}", COOKIE_JWT_TOKEN_NAME, jwt))
            .map_err(|e| AuthError::HeaderError(e.to_string()))?,
    );
    if let Some(csrf) = csrf_token {
        headers.insert(
            CSRF_TOKEN_NAME,
            HeaderValue::from_str(csrf).map_err(|e| AuthError::HeaderError(e.to_string()))?,
        );
    }
    Ok(headers)
}
