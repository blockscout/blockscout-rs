use alloy::primitives::{Address, B256};
use anyhow::{Context, Result, bail, ensure};

use super::version::HeaderLayout;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AmbHeader {
    pub(crate) packing_version: [u8; 4],
    pub(crate) message_id: B256,
    pub(crate) sender: Address,
    pub(crate) executor: Address,
    pub(crate) gas_limit: u32,
    pub(crate) data_type: u8,
    pub(crate) source_chain_id: i64,
    pub(crate) destination_chain_id: i64,
    pub(crate) payload_offset: usize,
}

pub(crate) fn parse_amb_header(encoded_data: &[u8], layout: HeaderLayout) -> Result<AmbHeader> {
    match layout {
        HeaderLayout::Modern => parse_modern_header(encoded_data),
        HeaderLayout::Legacy => bail!("legacy AMB header layout is not supported in v1"),
    }
}

fn parse_modern_header(encoded_data: &[u8]) -> Result<AmbHeader> {
    ensure!(
        encoded_data.len() >= 79,
        "AMB encodedData too short for modern header: {} bytes",
        encoded_data.len()
    );

    let packing_version = encoded_data[0..4].try_into()?;
    let message_id = B256::from_slice(&encoded_data[0..32]);
    let sender = Address::from_slice(&encoded_data[32..52]);
    let executor = Address::from_slice(&encoded_data[52..72]);
    let gas_limit = u32::from_be_bytes(encoded_data[72..76].try_into()?);
    let source_len = encoded_data[76] as usize;
    let destination_len = encoded_data[77] as usize;
    let data_type = encoded_data[78];
    let source_start = 79;
    let destination_start = source_start + source_len;
    let payload_offset = destination_start + destination_len;

    ensure!(
        encoded_data.len() >= payload_offset,
        "AMB encodedData too short for chain ids: {} bytes",
        encoded_data.len()
    );
    ensure!(
        source_len > 0 && source_len <= 8 && destination_len > 0 && destination_len <= 8,
        "unsupported AMB chain id lengths: source={source_len}, destination={destination_len}"
    );

    Ok(AmbHeader {
        packing_version,
        message_id,
        sender,
        executor,
        gas_limit,
        data_type,
        source_chain_id: parse_chain_id(&encoded_data[source_start..destination_start])
            .context("invalid source chain id")?,
        destination_chain_id: parse_chain_id(&encoded_data[destination_start..payload_offset])
            .context("invalid destination chain id")?,
        payload_offset,
    })
}

fn parse_chain_id(bytes: &[u8]) -> Result<i64> {
    let mut padded = [0u8; 8];
    padded[8 - bytes.len()..].copy_from_slice(bytes);
    let value = u64::from_be_bytes(padded);
    i64::try_from(value).context("chain id exceeds i64::MAX")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modern_header_extracts_chain_ids_and_payload_offset() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0, 5, 0, 0]);
        data.extend_from_slice(&[1u8; 28]);
        data.extend_from_slice(&[2u8; 20]);
        data.extend_from_slice(&[3u8; 20]);
        data.extend_from_slice(&100u32.to_be_bytes());
        data.push(1);
        data.push(1);
        data.push(0);
        data.push(1);
        data.push(100);
        data.extend_from_slice(&[0xaa, 0xbb]);

        let header = parse_amb_header(&data, HeaderLayout::Modern).unwrap();

        assert_eq!(header.packing_version, [0, 5, 0, 0]);
        assert_eq!(header.source_chain_id, 1);
        assert_eq!(header.destination_chain_id, 100);
        assert_eq!(header.payload_offset, 81);
        assert_eq!(&data[header.payload_offset..], &[0xaa, 0xbb]);
    }
}
