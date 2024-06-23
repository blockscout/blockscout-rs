pub mod chain_id;
pub mod chain_name;
pub mod chain_type;
pub mod instance_url;
pub mod is_testnet;
pub mod node_type;
pub mod rpc_url;
pub mod rpc_ws_url;
pub mod server_size;
pub mod stats_enabled;
pub mod token_symbol;

mod frontend;
pub use frontend::*;
