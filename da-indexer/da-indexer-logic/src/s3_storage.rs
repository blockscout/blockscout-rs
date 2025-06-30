use anyhow::Context;
use base64::{prelude::BASE64_STANDARD, Engine};
use futures::{stream::TryStreamExt, StreamExt};
use minio::{
    s3,
    s3::{
        multimap::{Multimap, MultimapExt},
        types::S3Api,
    },
};
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
        let credentials = s3::creds::StaticProvider::new(
            &settings.access_key_id,
            &settings.secret_access_key,
            None,
        );
        let client = s3::Client::new(
            settings
                .endpoint
                .parse()
                .context("parsing endpoint into url failed")?,
            Some(Box::new(credentials)),
            None,
            None,
        )
        .context("s3 client initialization failed")?;

        if settings.create_bucket {
            Self::create_bucket_if_not_exists(&client, &settings.bucket)
                .await
                .context("bucket initialization failed")?;
        }

        if settings.validate_on_initialization {
            let bucket_exists_response = client
                .bucket_exists(&settings.bucket)
                .send()
                .await
                .context("bucket validation failed")?;
            if !bucket_exists_response.exists {
                anyhow::bail!("bucket ({}) is not available", settings.bucket);
            }
        }

        Ok(S3Storage {
            client,
            bucket: settings.bucket,
            save_concurrency_limit: settings.save_concurrency_limit,
        })
    }

    pub async fn find_object_by_key(&self, key: &str) -> Result<Option<S3Object>, anyhow::Error> {
        let result = self.client.get_object(&self.bucket, key).send().await;
        match result {
            Ok(object_response) => {
                let content = object_response
                    .content
                    .to_segmented_bytes()
                    .await
                    .context("download object content")?;

                Ok(Some(S3Object {
                    key: key.to_string(),
                    content: content.to_bytes().to_vec(),
                }))
            }
            Err(s3::error::Error::S3Error(error))
                if error.code == s3::error::ErrorCode::NoSuchKey =>
            {
                Ok(None)
            }
            Err(error) => Err(error).context("get object by key"),
        }
    }

    pub async fn insert_many(&self, objects: Vec<S3Object>) -> Result<(), anyhow::Error> {
        futures::stream::iter(objects)
            .map(Ok)
            .try_for_each_concurrent(self.save_concurrency_limit, |object| async move {
                let content_md5 = md5::compute(&object.content);

                let mut content = s3::segmented_bytes::SegmentedBytes::new();
                content.append(object.content.into());

                let mut extra_headers = Multimap::new();
                extra_headers.add("Content-MD5", BASE64_STANDARD.encode(content_md5.0));

                let response = self
                    .client
                    .put_object(&self.bucket, &object.key, content)
                    .extra_headers(Some(extra_headers))
                    .send()
                    .await
                    .context(format!("put object {} into s3 storage failed", object.key))?;

                // Have to always be false, as the integrity should already be validated
                // by S3 storage as we provided 'Content-MD5' header. But added additional
                // check here just in case the header is not supported by the given storage.
                if response.etag != hex::encode(content_md5.0) {
                    anyhow::bail!(
                        "({}) object MD5 checksum does not match returned ETag value",
                        object.key
                    )
                }

                Ok(())
            })
            .await
    }

    async fn create_bucket_if_not_exists(
        s3_client: &s3::Client,
        bucket_name: &str,
    ) -> Result<(), anyhow::Error> {
        let result = s3_client.create_bucket(bucket_name).send().await;
        match result {
            Ok(_) => Ok(()),
            Err(s3::error::Error::S3Error(error))
                if error.code == s3::error::ErrorCode::BucketAlreadyOwnedByYou =>
            {
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }
}
