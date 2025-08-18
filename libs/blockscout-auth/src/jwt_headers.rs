use crate::auth::Error;
use cookie::Cookie;
use reqwest::header::{HeaderMap, HeaderValue};
use tonic::metadata::MetadataMap;

pub const HEADER_JWT_TOKEN_NAME: &str = "authorization";
pub const COOKIE_JWT_TOKEN_NAME: &str = "_explorer_key";
pub const CSRF_TOKEN_NAME: &str = "x-csrf-token";
pub const HEADER_COOKIE_NAME: &str = "cookie";

pub fn extract_jwt(metadata: &MetadataMap) -> Result<String, Error> {
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
        .ok_or_else(|| Error::InvalidJwt("jwt not found".into()))
}

pub fn extract_csrf_token(metadata: &MetadataMap) -> Result<String, Error> {
    metadata
        .get(CSRF_TOKEN_NAME)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::InvalidCsrfToken("csrf not found".into()))
}

fn get_cookies(
    metadata: &MetadataMap,
) -> Result<std::collections::HashMap<String, Cookie<'_>>, Error> {
    let raw = metadata
        .get(HEADER_COOKIE_NAME)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    Cookie::split_parse_encoded(raw.to_string())
        .map(|r| r.map(|c| (c.name().to_string(), c)))
        .collect::<Result<_, _>>()
        .map_err(|e| Error::InvalidJwt(format!("cannot parse cookie: {e}")))
}

pub fn build_http_headers(jwt: &str, csrf_token: Option<&str>) -> Result<HeaderMap, Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_str(&format!("{COOKIE_JWT_TOKEN_NAME}={jwt}"))
            .map_err(|e| Error::HeaderError(e.to_string()))?,
    );
    if let Some(csrf) = csrf_token {
        headers.insert(
            CSRF_TOKEN_NAME,
            HeaderValue::from_str(csrf).map_err(|e| Error::HeaderError(e.to_string()))?,
        );
    }
    Ok(headers)
}
