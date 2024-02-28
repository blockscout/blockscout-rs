use blockscout_service_launcher::launcher;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub database_url: String,

    pub proxy_verifier_url: url::Url,

    #[serde(default = "default_n_threads")]
    pub n_threads: usize,
}

impl launcher::ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "ALLIANCE_EXTRACTOR";
}

fn default_n_threads() -> usize {
    1
}
