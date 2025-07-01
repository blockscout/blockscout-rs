use crate::metrics;
use anyhow::Context;
use aws_credential_types::Credentials;
use aws_sdk_s3::{self as s3, config::Region};
use base64::{prelude::BASE64_STANDARD, Engine};
use futures::{stream::TryStreamExt, StreamExt};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct S3StorageSettings {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub save_concurrency_limit: Option<usize>,
    #[serde(default)]
    pub create_bucket: bool,
    #[serde(default)]
    pub validate_on_initialization: bool,
}

#[derive(Clone, Debug)]
pub struct S3Storage {
    // Use one bucket instance for all users
    client: s3::Client,
    bucket: String,
    save_concurrency_limit: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct S3Object {
    pub key: String,
    pub content: Vec<u8>,
}

impl S3Storage {
    pub async fn new(settings: S3StorageSettings) -> anyhow::Result<Self> {
        let credentials =
            Credentials::from_keys(&settings.access_key_id, &settings.secret_access_key, None);
        let config = s3::Config::builder()
            .endpoint_url(&settings.endpoint)
            .credentials_provider(credentials)
            .region(Region::new(""))
            .force_path_style(true) // required to use minio as underlying s3 storage provider
            .build();
        let client = s3::Client::from_conf(config);

        if settings.create_bucket {
            Self::create_bucket_if_not_exists(&client, &settings.bucket)
                .await
                .context("bucket initialization failed")?;
        }

        if settings.validate_on_initialization {
            Self::validate_bucket(&client, &settings.bucket)
                .await
                .context("bucket validation failed")?;
        }

        Ok(S3Storage {
            client,
            bucket: settings.bucket,
            save_concurrency_limit: settings.save_concurrency_limit,
        })
    }

    pub async fn find_object_by_key(&self, key: &str) -> Result<Option<S3Object>, anyhow::Error> {
        let result = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await;
        match result {
            Ok(object_response) => {
                let content = object_response
                    .body
                    .collect()
                    .await
                    .context("download object content failed")?
                    .to_vec();

                let e_tag = object_response.e_tag.clone();
                let content_md5 = Self::calculate_content_md5(&content);
                Self::validate_object_e_tag(key, e_tag.as_deref(), content_md5)?;

                Ok(Some(S3Object {
                    key: key.to_string(),
                    content,
                }))
            }
            Err(error) => {
                if let Some(error) = error.as_service_error() {
                    if error.is_no_such_key() {
                        return Ok(None);
                    }
                }
                Err(error).context("get object by key failed")
            }
        }
    }

    pub async fn insert_many(&self, objects: Vec<S3Object>) -> Result<(), anyhow::Error> {
        metrics::S3_BULK_UPLOAD_TOTAL.inc();
        {
            let _timer = metrics::S3_BULK_UPLOAD_TIME.start_timer();
            futures::stream::iter(objects)
                .map(Ok::<_, anyhow::Error>)
                .try_for_each_concurrent(self.save_concurrency_limit, |object| async move {
                    let content_md5 = Self::calculate_content_md5(&object.content);

                    let body = s3::primitives::ByteStream::from(object.content);
                    let response = self
                        .client
                        .put_object()
                        .bucket(&self.bucket)
                        .key(&object.key)
                        .body(body)
                        .content_md5(BASE64_STANDARD.encode(content_md5))
                        .send()
                        .await
                        .context(format!("put object {} into s3 storage failed", object.key))?;

                    // Have to always be false, as the integrity should already be validated
                    // by S3 storage as we provided 'Content-MD5' header. But added additional
                    // check here just in case the header is not supported by the given storage.
                    Self::validate_object_e_tag(&object.key, response.e_tag(), content_md5)?;

                    Ok(())
                })
                .await?;
        }
        metrics::S3_BULK_UPLOAD_SUCCESSES.inc();
        Ok(())
    }

    async fn create_bucket_if_not_exists(
        s3_client: &s3::Client,
        bucket_name: &str,
    ) -> Result<(), anyhow::Error> {
        let result = s3_client.create_bucket().bucket(bucket_name).send().await;
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                if let Some(error) = error.as_service_error() {
                    if error.is_bucket_already_exists() || error.is_bucket_already_owned_by_you() {
                        tracing::info!(
                            bucket = bucket_name,
                            "trying to create a new bucket; bucket already exists"
                        );
                        return Ok(());
                    }
                }
                Err(error.into())
            }
        }
    }

    async fn validate_bucket(
        s3_client: &s3::Client,
        bucket_name: &str,
    ) -> Result<(), anyhow::Error> {
        // Here we assume that the bucket exists and we have an access to it
        // if we can list the bucket objects.
        let _list_object_output = s3_client
            .list_objects()
            .bucket(bucket_name)
            .max_keys(1)
            .send()
            .await?;
        Ok(())
    }

    fn calculate_content_md5(content: &[u8]) -> [u8; 16] {
        md5::compute(content).0
    }

    fn validate_object_e_tag(
        object_key: &str,
        e_tag: Option<&str>,
        content_md5: [u8; 16],
    ) -> Result<(), anyhow::Error> {
        if let Some(e_tag) = e_tag {
            // For some reason, aws-sdk-s3 returns e-tag as a string wrapped in '"' ("\"123...\"")
            let e_tag = e_tag.trim_start_matches('"').trim_end_matches('"');
            if e_tag != hex::encode(content_md5) {
                anyhow::bail!(
                    "({}) object MD5 checksum does not match returned ETag value",
                    object_key
                )
            }
        }
        Ok(())
    }
}
