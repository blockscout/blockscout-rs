// SPDX-License-Identifier: LicenseRef-Blockscout

//! Reads the deployment transaction from the caller's RPC endpoint.
//!
//! The endpoint comes from the request, so the service must not be turned into a way to reach
//! its own network: the host has to resolve to a public address, the connection goes to exactly
//! the address that was checked, and redirects are not followed.

use crate::stylus_sdk_rs::Error;
use anyhow::Context;
use blockscout_display_bytes::ToHex;
use bytes::Bytes;
use serde::Deserialize;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeploymentTransaction {
    pub input: Bytes,
    /// False when the transaction calls a contract instead of creating one.
    pub creates_contract: bool,
}

pub(crate) async fn fetch(
    rpc_endpoint: &str,
    transaction_hash: &[u8],
    allow_private_addresses: bool,
    timeout: Duration,
) -> Result<DeploymentTransaction, Error> {
    let url = parse_endpoint(rpc_endpoint)?;
    let port = url.port_or_known_default().unwrap_or(443);
    let acceptable = |address: &SocketAddr| allow_private_addresses || is_public(address.ip());
    let not_public =
        || Error::BadRequest("rpc_endpoint must resolve to a public address".to_string());
    let mut client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout);
    match url.host() {
        Some(url::Host::Domain(domain)) => {
            let address = tokio::net::lookup_host((domain, port))
                .await
                .map_err(|error| {
                    Error::BadRequest(format!("rpc_endpoint cannot be resolved: {error}"))
                })?
                .find(acceptable)
                .ok_or_else(not_public)?;
            // The request goes to the address checked here, whatever the name resolves to later.
            client = client.resolve(domain, address);
        }
        Some(url::Host::Ipv4(address)) if acceptable(&SocketAddr::new(address.into(), port)) => {}
        Some(url::Host::Ipv6(address)) if acceptable(&SocketAddr::new(address.into(), port)) => {}
        _ => return Err(not_public()),
    }
    let client = client.build().context("failed to build the rpc client")?;
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getTransactionByHash",
        "params": [ToHex::to_hex(&transaction_hash)],
    });
    let unreachable =
        |error: reqwest::Error| Error::BadRequest(format!("rpc_endpoint request failed: {error}"));
    let mut response = client
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(unreachable)?;
    if !response.status().is_success() {
        return Err(Error::BadRequest(format!(
            "rpc_endpoint answered with status {}",
            response.status()
        )));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(unreachable)? {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(Error::BadRequest(
                "rpc_endpoint response is too large".to_string(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    parse_response(&body)
}

fn parse_endpoint(rpc_endpoint: &str) -> Result<Url, Error> {
    let url = Url::parse(rpc_endpoint)
        .map_err(|error| Error::BadRequest(format!("rpc_endpoint is not a valid url: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Error::BadRequest(
            "rpc_endpoint must be an http or https url".to_string(),
        ));
    }
    Ok(url)
}

fn parse_response(body: &[u8]) -> Result<DeploymentTransaction, Error> {
    #[derive(Deserialize)]
    struct Response {
        result: Option<Transaction>,
        error: Option<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Transaction {
        input: String,
        to: Option<String>,
    }

    let response: Response = serde_json::from_slice(body).map_err(|error| {
        Error::BadRequest(format!(
            "rpc_endpoint did not answer with json-rpc: {error}"
        ))
    })?;
    if let Some(error) = response.error {
        return Err(Error::BadRequest(format!(
            "rpc_endpoint returned an error: {error}"
        )));
    }
    let transaction = response
        .result
        .ok_or_else(|| Error::BadRequest("deployment transaction not found".to_string()))?;
    let input = blockscout_display_bytes::decode_hex(&transaction.input).map_err(|error| {
        Error::BadRequest(format!("rpc_endpoint returned invalid input data: {error}"))
    })?;
    Ok(DeploymentTransaction {
        input: input.into(),
        creates_contract: transaction.to.is_none(),
    })
}

/// Whether an address is outside every range that is only reachable from inside a network.
fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_v4(address),
        IpAddr::V6(address) => match address.to_ipv4_mapped() {
            Some(mapped) => is_public_v4(mapped),
            None => is_public_v6(address),
        },
    }
}

fn is_public_v4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_broadcast()
        || address.is_multicast()
        || address.is_documentation()
        // "This network", carrier-grade NAT, protocol assignments, benchmarking and reserved.
        || octets[0] == 0
        || (octets[0] == 100 && (64..128).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && (18..20).contains(&octets[1]))
        || octets[0] >= 240)
}

fn is_public_v6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    // NAT64 (64:ff9b::/96) reaches the IPv4 address in its last 32 bits.
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        let [.., high, low] = segments;
        return is_public_v4(Ipv4Addr::from((u32::from(high) << 16) | u32::from(low)));
    }
    let first = segments[0];
    !(address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        // Unique local and link-local.
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_addresses_reachable_from_outside_are_public() {
        for address in [
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "127.0.0.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
            "::",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "ff02::1",
            "::ffff:10.0.0.1",
            "::ffff:127.0.0.1",
            "64:ff9b::a00:1",
            "64:ff9b::7f00:1",
        ] {
            assert!(!is_public(address.parse().unwrap()), "{address} is public");
        }
        for address in [
            "1.1.1.1",
            "140.82.121.4",
            "2606:4700:4700::1111",
            "64:ff9b::101:101",
        ] {
            assert!(is_public(address.parse().unwrap()), "{address} is private");
        }
    }

    #[test]
    fn endpoint_must_be_an_http_url_with_a_host() {
        for endpoint in [
            "",
            "rpc",
            "file:///etc/passwd",
            "ftp://example.com",
            "unix:///x",
        ] {
            assert!(
                matches!(parse_endpoint(endpoint), Err(Error::BadRequest(_))),
                "{endpoint:?} was accepted"
            );
        }
        parse_endpoint("https://sepolia-rollup.arbitrum.io/rpc").unwrap();
        parse_endpoint("http://node.example.com:8545").unwrap();
    }

    #[tokio::test]
    async fn private_endpoints_are_refused_before_any_request() {
        for endpoint in [
            "http://127.0.0.1:8545",
            "http://[::1]:8545",
            "http://169.254.169.254/latest",
            "http://localhost:8545",
        ] {
            let result = fetch(endpoint, &[0; 32], false, Duration::from_secs(2)).await;
            match result {
                Err(Error::BadRequest(message)) => {
                    assert!(message.contains("public address"), "{endpoint}: {message}")
                }
                other => panic!("{endpoint} was not refused: {other:?}"),
            }
        }
    }

    #[test]
    fn response_yields_the_input_and_whether_it_creates_a_contract() {
        let creation = parse_response(br#"{"result":{"input":"0x7f00ef","to":null}}"#).unwrap();
        assert_eq!(creation.input, Bytes::from_static(&[0x7f, 0x00, 0xef]));
        assert!(creation.creates_contract);

        let call = parse_response(br#"{"result":{"input":"0x01","to":"0xabc"}}"#).unwrap();
        assert!(!call.creates_contract);

        for body in [
            &br#"{"result":null}"#[..],
            br#"{"error":{"code":-32000,"message":"boom"}}"#,
            br#"{"result":{"input":"zz","to":null}}"#,
            b"<html>",
        ] {
            assert!(matches!(parse_response(body), Err(Error::BadRequest(_))));
        }
    }
}
