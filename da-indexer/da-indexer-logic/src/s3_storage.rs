use anyhow::Context;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct S3StorageSettings {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default)]
    pub validate_on_initialization: bool,
}

#[derive(Clone, Debug)]
pub struct S3Storage {
    // Use one bucket instance for all users
    _bucket: Arc<s3::Bucket>,
}

impl S3Storage {
    pub async fn new(settings: S3StorageSettings) -> anyhow::Result<Self> {
        let region = s3::Region::Custom {
            region: "custom".to_string(),
            endpoint: settings.endpoint,
        };
        let bucket = s3::Bucket::new(
            &settings.bucket,
            region,
            s3::creds::Credentials::new(
                Some(&settings.access_key_id),
                Some(&settings.secret_access_key),
                None,
                None,
                None,
            )
            .context("credentials initialization failed")?,
        )
        .context("bucket initialization failed")?;

        if settings.validate_on_initialization
            && !bucket.exists().await.context("bucket validation failed")?
        {
            anyhow::bail!("bucket ({}) is not available", settings.bucket);
        }

        Ok(S3Storage {
            _bucket: bucket.into(),
        })
    }
}
