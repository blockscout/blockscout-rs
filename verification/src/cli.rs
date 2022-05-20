use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct CLIArgs {
    #[clap(short, long, default_value = "0.0.0.0")]
    pub address: String,
    #[clap(short, long, default_value = "8043")]
    pub port: u16,
}

pub fn parse_args() -> CLIArgs {
    CLIArgs::parse()
}
