use sol_to_uml::{config::Config, run};
use std::process::Command;
use regex::Regex;

#[derive(PartialEq, Default, Clone, Debug)]
struct Commit {
    hash: String,
    message: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Config::parse().expect("Failed to parse config");
    run(config).await
}