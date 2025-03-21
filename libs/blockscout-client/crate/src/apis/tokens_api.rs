/*
 * BlockScout API
 *
 * API for BlockScout web app
 *
 * The version of the OpenAPI document: 1.0.0
 * Contact: you@your-company.com
 * Generated by: https://openapi-generator.tech
 */

use super::{configuration, Error};
use crate::{apis::ResponseContent, models};
use reqwest;
use serde::{Deserialize, Serialize};

/// struct for typed errors of method [`get_nft_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetNftInstanceError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_nft_instance_transfers`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetNftInstanceTransfersError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_nft_instance_transfers_count`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetNftInstanceTransfersCountError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_nft_instances`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetNftInstancesError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_token`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetTokenError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_token_counters`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetTokenCountersError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_token_holders`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetTokenHoldersError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_token_instance_holders`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetTokenInstanceHoldersError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_token_token_transfers`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetTokenTokenTransfersError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_tokens_list`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetTokensListError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`refetch_token_instance_metadata`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RefetchTokenInstanceMetadataError {
    Status403(models::RefetchTokenInstanceMetadata403Response),
    UnknownValue(serde_json::Value),
}

pub async fn get_nft_instance(
    configuration: &configuration::Configuration,
    address_hash: &str,
    id: i32,
) -> Result<models::NftInstance, Error<GetNftInstanceError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_id = id;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/instances/{id}",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash),
        id = p_id
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetNftInstanceError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_nft_instance_transfers(
    configuration: &configuration::Configuration,
    address_hash: &str,
    id: i32,
) -> Result<models::GetNftInstanceTransfers200Response, Error<GetNftInstanceTransfersError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_id = id;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/instances/{id}/transfers",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash),
        id = p_id
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetNftInstanceTransfersError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_nft_instance_transfers_count(
    configuration: &configuration::Configuration,
    address_hash: &str,
    id: i32,
) -> Result<models::GetNftInstanceTransfersCount200Response, Error<GetNftInstanceTransfersCountError>>
{
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_id = id;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/instances/{id}/transfers-count",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash),
        id = p_id
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetNftInstanceTransfersCountError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_nft_instances(
    configuration: &configuration::Configuration,
    address_hash: &str,
) -> Result<models::GetNftInstances200Response, Error<GetNftInstancesError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/instances",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetNftInstancesError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_token(
    configuration: &configuration::Configuration,
    address_hash: &str,
) -> Result<models::TokenInfo, Error<GetTokenError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetTokenError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_token_counters(
    configuration: &configuration::Configuration,
    address_hash: &str,
) -> Result<models::TokenCounters, Error<GetTokenCountersError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/counters",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetTokenCountersError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_token_holders(
    configuration: &configuration::Configuration,
    address_hash: &str,
) -> Result<models::GetTokenHolders200Response, Error<GetTokenHoldersError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/holders",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetTokenHoldersError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_token_instance_holders(
    configuration: &configuration::Configuration,
    address_hash: &str,
    id: i32,
) -> Result<models::GetTokenInstanceHolders200Response, Error<GetTokenInstanceHoldersError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_id = id;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/instances/{id}/holders",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash),
        id = p_id
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetTokenInstanceHoldersError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_token_token_transfers(
    configuration: &configuration::Configuration,
    address_hash: &str,
) -> Result<models::GetTokenTokenTransfers200Response, Error<GetTokenTokenTransfersError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/transfers",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetTokenTokenTransfersError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_tokens_list(
    configuration: &configuration::Configuration,
    q: Option<&str>,
    r#type: Option<&str>,
) -> Result<models::GetTokensList200Response, Error<GetTokensListError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_q = q;
    let p_type = r#type;

    let uri_str = format!("{}/api/v2/tokens", configuration.base_path);
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref param_value) = p_q {
        req_builder = req_builder.query(&[("q", &param_value.to_string())]);
    }
    if let Some(ref param_value) = p_type {
        req_builder = req_builder.query(&[("type", &param_value.to_string())]);
    }
    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetTokensListError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn refetch_token_instance_metadata(
    configuration: &configuration::Configuration,
    address_hash: &str,
    id: i32,
    recaptcha_body: models::RecaptchaBody,
) -> Result<models::RefetchTokenInstanceMetadata200Response, Error<RefetchTokenInstanceMetadataError>>
{
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_id = id;
    let p_recaptcha_body = recaptcha_body;

    let uri_str = format!(
        "{}/api/v2/tokens/{address_hash}/instances/{id}/refetch-metadata",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash),
        id = p_id
    );
    let mut req_builder = configuration
        .client
        .request(reqwest::Method::PATCH, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    req_builder = req_builder.json(&p_recaptcha_body);

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<RefetchTokenInstanceMetadataError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}
