use blockscout_service_launcher::test_database::TestDbGuard;

pub async fn init_db(test_name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(test_name).await
}

pub use s3_storage::{initialize_s3_storage, is_s3_storage_empty};
mod s3_storage {
    use crate::s3_storage::{S3Storage, S3StorageSettings};
    use aws_credential_types::Credentials;
    use aws_sdk_s3::{self as s3, config::Region};

    struct ClientWithDetails {
        endpoint: String,
        access_key_id: String,
        secret_access_key: String,
        bucket_name: String,
        client: s3::Client,
    }

    pub async fn initialize_s3_storage(test_name: &str) -> S3Storage {
        let client_with_details = initialize_client(test_name).await;
        initialize_bucket(
            &client_with_details.client,
            &client_with_details.bucket_name,
        )
        .await;

        S3Storage::new(S3StorageSettings {
            endpoint: client_with_details.endpoint,
            bucket: client_with_details.bucket_name,
            access_key_id: client_with_details.access_key_id,
            secret_access_key: client_with_details.secret_access_key,
            save_concurrency_limit: None,
            create_bucket: false,
            validate_on_initialization: true,
        })
        .await
        .expect("cannot initialize s3 storage")
    }

    pub async fn is_s3_storage_empty(test_name: &str) -> bool {
        let client_with_details = initialize_client(test_name).await;
        is_bucket_empty(
            &client_with_details.client,
            &client_with_details.bucket_name,
        )
        .await
    }

    async fn initialize_client(test_name: &str) -> ClientWithDetails {
        let endpoint =
            std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());
        let access_key_id =
            std::env::var("S3_ACCESS_KEY_ID").unwrap_or_else(|_| "minioadmin".to_string());
        let secret_access_key =
            std::env::var("S3_SECRET_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string());

        // Bucket names can consist only of lowercase letters, numbers, periods (.), and hyphens (-)
        let bucket_name = test_name.replace("_", "-");

        let credentials = Credentials::from_keys(&access_key_id, &secret_access_key, None);
        let region = Region::new("custom-region");
        let config = s3::Config::builder()
            .endpoint_url(&endpoint)
            .credentials_provider(credentials)
            .region(Some(region))
            .force_path_style(true)
            .build();

        let client = s3::Client::from_conf(config);

        ClientWithDetails {
            endpoint,
            access_key_id,
            secret_access_key,
            bucket_name,
            client,
        }
    }

    async fn initialize_bucket(client: &s3::Client, bucket_name: &str) {
        delete_bucket_if_exists(client, bucket_name).await;
        client
            .create_bucket()
            .bucket(bucket_name)
            .send()
            .await
            .expect("failed to create bucket");
    }

    async fn delete_bucket_if_exists(client: &s3::Client, bucket_name: &str) {
        if !does_bucket_exist(client, bucket_name).await {
            return;
        }
        empty_bucket(client, bucket_name).await;
        client
            .delete_bucket()
            .bucket(bucket_name)
            .send()
            .await
            .expect("failed to delete existing bucket");
    }

    async fn empty_bucket(client: &s3::Client, bucket_name: &str) {
        let mut objects = client
            .list_objects_v2()
            .bucket(bucket_name)
            .into_paginator()
            .send();

        while let Some(Ok(page)) = objects.next().await {
            let keys_to_delete = page
                .contents
                .unwrap_or_default()
                .into_iter()
                .filter_map(|object| object.key);
            for key in keys_to_delete {
                client
                    .delete_object()
                    .bucket(bucket_name)
                    .key(&key)
                    .send()
                    .await
                    .expect("failed to delete object");
            }
        }
    }

    async fn does_bucket_exist(client: &s3::Client, bucket_name: &str) -> bool {
        client
            .get_bucket_acl()
            .bucket(bucket_name)
            .send()
            .await
            .is_ok()
    }

    async fn is_bucket_empty(client: &s3::Client, bucket_name: &str) -> bool {
        let page = client
            .list_objects_v2()
            .bucket(bucket_name)
            .max_keys(1)
            .send()
            .await
            .expect("failed to list objects");
        page.key_count.unwrap() == 0
    }
}
