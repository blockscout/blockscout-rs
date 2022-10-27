mod server;
mod types;

pub use crate::proto::blockscout::visualizer::v1::solidity_visualizer_actix::route_solidity_visualizer;
pub use server::SolidityVisualizerService;
