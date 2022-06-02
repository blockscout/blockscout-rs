use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short, long, default_value = "config.toml")]
    pub config_path: std::path::PathBuf,
}
