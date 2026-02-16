use anyhow::{Context, Result, anyhow};
use interchain_indexer_logic::avalanche_data_api::{
    AvalancheDataApiClient, AvalancheDataApiClientSettings, AvalancheDataApiNetwork,
};

fn usage() -> &'static str {
    "Usage:\n  cargo resolve-blockchain-id <blockchain_id> [--network mainnet|fuji|testnet]\n\nExamples:\n  cargo resolve-blockchain-id 0x4d17dde28b48d8261d0e157ff214900e6575600325f02d6efb416eebdbde4ba9\n  cargo resolve-blockchain-id 2DzcZrV... --network fuji\n\nEnv:\n  AVALANCHE_DATA_API_NETWORK=mainnet|fuji|testnet\n  AVALANCHE_GLACIER_API_KEY=...   (optional)\n  AVALANCHE_DATA_API_KEY=...      (optional)\n"
}

fn parse_args() -> Result<(String, AvalancheDataApiNetwork)> {
    let mut args = std::env::args().skip(1);

    let Some(id) = args.next() else {
        return Err(anyhow!("missing <blockchain_id>\n\n{}", usage()));
    };

    let mut network = AvalancheDataApiNetwork::default();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--network" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("--network requires a value\n\n{}", usage()))?;
                network = AvalancheDataApiNetwork::try_from(v.as_str()).map_err(|_| {
                    anyhow!(
                        "unknown network: {v}. expected mainnet|fuji|testnet\n\n{}",
                        usage()
                    )
                })?;
            }
            "-h" | "--help" => {
                return Err(anyhow!("{}", usage()));
            }
            other => {
                return Err(anyhow!("unknown argument: {other}\n\n{}", usage()));
            }
        }
    }

    Ok((id, network))
}

fn parse_blockchain_id(id: &str) -> Result<[u8; 32]> {
    let id = id.trim();

    if let Some(hex_str) = id.strip_prefix("0x") {
        let bytes = hex::decode(hex_str).context("failed to decode hex blockchain id")?;
        let len = bytes.len();
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow!("expected 32-byte blockchain_id, got {}", len))?;
        return Ok(arr);
    }

    // Treat as CB58 string.
    let decoded = bs58::decode(id)
        .as_cb58(None)
        .into_vec()
        .context("failed to decode CB58 blockchain id")?;

    let len = decoded.len();
    let arr: [u8; 32] = decoded
        .try_into()
        .map_err(|_| anyhow!("expected 32-byte blockchain_id, got {}", len))?;

    Ok(arr)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Best-effort logging for retry warnings.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let (id, network) = parse_args()?;
    let bytes = parse_blockchain_id(&id)?;
    let cb58 = bs58::encode(bytes).as_cb58(None).into_string();

    let mut settings = AvalancheDataApiClientSettings::default();
    settings.network = network;

    let data_api = AvalancheDataApiClient::from_settings(settings);
    let resp = data_api.get_blockchain_by_id(&bytes).await?;

    println!("network:            {}", network.as_ref());
    println!("blockchainId(cb58): {}", cb58);
    println!("blockchainId(hex):  0x{}", hex::encode(bytes));
    println!("blockchainName:     {}", resp.blockchain_name);
    match resp.evm_chain_id {
        Some(chain_id) => println!("evmChainId:         {}", chain_id),
        None => println!("evmChainId:         <none>"),
    }

    Ok(())
}
