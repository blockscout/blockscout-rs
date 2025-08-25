// check-envs.rs
use zetachain_cctx_server::Settings;
use env_collector::{run_env_collector_cli, PrefixFilter};

fn main() {

    run_env_collector_cli::<Settings>(
        "ZETACHAIN_CCTX",
        "README.md",
        "config/example.toml",
        PrefixFilter::blacklist(&[
            "ZETACHAIN_CCTX__SERVER",
            "ZETACHAIN_CCTX__TRACING",
            "ZETACHAIN_CCTX__JAEGER",
            "ZETACHAIN_CCTX__METRICS",
        ]),
        None,
    );
}