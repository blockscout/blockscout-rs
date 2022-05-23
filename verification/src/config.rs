use clap::Parser;
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use crate::cli;

pub struct Config {
    pub socket_addr: SocketAddr,
}

impl Config {
    pub fn parse() -> Self {
        let args = cli::Args::parse();
        let addr = IpAddr::from_str(args.address.as_str()).unwrap();
        Config {
            socket_addr: SocketAddr::from((addr, args.port)),
        }
    }
}
