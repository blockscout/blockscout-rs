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

/// struct for typed errors of method [`get_read_methods`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetReadMethodsError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_read_methods_proxy`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetReadMethodsProxyError {
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_smart_contract`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetSmartContractError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_smart_contracts`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetSmartContractsError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_smart_contracts_counters`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetSmartContractsCountersError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_write_methods`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetWriteMethodsError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_write_methods_proxy`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetWriteMethodsProxyError {
    Status400(),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`query_read_method`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryReadMethodError {
    Status400(),
    UnknownValue(serde_json::Value),
}

pub async fn get_read_methods(
    configuration: &configuration::Configuration,
    address_hash: &str,
    is_custom_abi: Option<&str>,
    from: Option<&str>,
) -> Result<Vec<models::GetReadMethods200ResponseInner>, Error<GetReadMethodsError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_is_custom_abi = is_custom_abi;
    let p_from = from;

    let uri_str = format!(
        "{}/api/v2/smart-contracts/{address_hash}/methods-read",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref param_value) = p_is_custom_abi {
        req_builder = req_builder.query(&[("is_custom_abi", &param_value.to_string())]);
    }
    if let Some(ref param_value) = p_from {
        req_builder = req_builder.query(&[("from", &param_value.to_string())]);
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
        let entity: Option<GetReadMethodsError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_read_methods_proxy(
    configuration: &configuration::Configuration,
    address_hash: &str,
    is_custom_abi: Option<&str>,
    from: Option<&str>,
) -> Result<Vec<models::GetReadMethods200ResponseInner>, Error<GetReadMethodsProxyError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_is_custom_abi = is_custom_abi;
    let p_from = from;

    let uri_str = format!(
        "{}/api/v2/smart-contracts/{address_hash}/methods-read-proxy",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref param_value) = p_is_custom_abi {
        req_builder = req_builder.query(&[("is_custom_abi", &param_value.to_string())]);
    }
    if let Some(ref param_value) = p_from {
        req_builder = req_builder.query(&[("from", &param_value.to_string())]);
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
        let entity: Option<GetReadMethodsProxyError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_smart_contract(
    configuration: &configuration::Configuration,
    address_hash: &str,
) -> Result<models::SmartContract, Error<GetSmartContractError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;

    let uri_str = format!(
        "{}/api/v2/smart-contracts/{address_hash}",
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
        let entity: Option<GetSmartContractError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_smart_contracts(
    configuration: &configuration::Configuration,
    q: Option<&str>,
    filter: Option<&str>,
) -> Result<models::GetSmartContracts200Response, Error<GetSmartContractsError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_q = q;
    let p_filter = filter;

    let uri_str = format!("{}/api/v2/smart-contracts", configuration.base_path);
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref param_value) = p_q {
        req_builder = req_builder.query(&[("q", &param_value.to_string())]);
    }
    if let Some(ref param_value) = p_filter {
        req_builder = req_builder.query(&[("filter", &param_value.to_string())]);
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
        let entity: Option<GetSmartContractsError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_smart_contracts_counters(
    configuration: &configuration::Configuration,
) -> Result<models::GetSmartContractsCounters200Response, Error<GetSmartContractsCountersError>> {
    let uri_str = format!(
        "{}/api/v2/smart-contracts/counters",
        configuration.base_path
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
        let entity: Option<GetSmartContractsCountersError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_write_methods(
    configuration: &configuration::Configuration,
    address_hash: &str,
    is_custom_abi: Option<&str>,
) -> Result<Vec<models::GetWriteMethods200ResponseInner>, Error<GetWriteMethodsError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_is_custom_abi = is_custom_abi;

    let uri_str = format!(
        "{}/api/v2/smart-contracts/{address_hash}/methods-write",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref param_value) = p_is_custom_abi {
        req_builder = req_builder.query(&[("is_custom_abi", &param_value.to_string())]);
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
        let entity: Option<GetWriteMethodsError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn get_write_methods_proxy(
    configuration: &configuration::Configuration,
    address_hash: &str,
    is_custom_abi: Option<&str>,
) -> Result<Vec<models::GetWriteMethods200ResponseInner>, Error<GetWriteMethodsProxyError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_is_custom_abi = is_custom_abi;

    let uri_str = format!(
        "{}/api/v2/smart-contracts/{address_hash}/methods-write-proxy",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref param_value) = p_is_custom_abi {
        req_builder = req_builder.query(&[("is_custom_abi", &param_value.to_string())]);
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
        let entity: Option<GetWriteMethodsProxyError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

pub async fn query_read_method(
    configuration: &configuration::Configuration,
    address_hash: &str,
    read_method_query_body: models::ReadMethodQueryBody,
) -> Result<Vec<models::QueryReadMethod200ResponseInner>, Error<QueryReadMethodError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_address_hash = address_hash;
    let p_read_method_query_body = read_method_query_body;

    let uri_str = format!(
        "{}/api/v2/smart-contracts/{address_hash}/query-read-method",
        configuration.base_path,
        address_hash = crate::apis::urlencode(p_address_hash)
    );
    let mut req_builder = configuration
        .client
        .request(reqwest::Method::POST, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    req_builder = req_builder.json(&p_read_method_query_body);

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<QueryReadMethodError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}
