use clap::Parser;
use std::net::SocketAddr;
use url::Url;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the config file that describes config.rs structure.
    #[clap(long)]
    pub config_path: Option<std::path::PathBuf>,

    /// The base URL of the Blockscout API.
    #[clap(long)]
    pub base_url: Option<Url>,

    /// The base URL of the Blockscout API.
    #[clap(long)]
    pub concurrent_requests: Option<usize>,

    /// Socket address of the server.
    #[clap(long)]
    pub address: Option<SocketAddr>,
}
