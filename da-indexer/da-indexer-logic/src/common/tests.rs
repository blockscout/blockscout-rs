use blockscout_service_launcher::test_database::TestDbGuard;

pub async fn init_db(test_name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(test_name).await
}

pub use s3_storage::{initialize_s3_storage, is_s3_storage_empty};
mod s3_storage {
    use crate::s3_storage::{S3Storage, S3StorageSettings};
    use futures::StreamExt;
    use minio::s3::{
        self,
        builders::ObjectToDelete,
        creds::StaticProvider,
        response::ListObjectsResponse,
        types::{S3Api, ToStream},
    };

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

        let credentials = StaticProvider::new(&access_key_id, &secret_access_key, None);
        let client = s3::Client::new(
            endpoint.parse().expect("cannot parse endpoint as url"),
            Some(Box::new(credentials)),
            None,
            None,
        )
        .expect("cannot initialize s3 client");

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
            .create_bucket(bucket_name)
            .send()
            .await
            .expect("failed to create bucket");
    }

    async fn delete_bucket_if_exists(client: &s3::Client, bucket_name: &str) {
        let bucket_exists = client
            .bucket_exists(bucket_name)
            .send()
            .await
            .expect("failed to check if bucket exists");
        if !bucket_exists.exists {
            return;
        }

        delete_bucket_objects(client, bucket_name).await;
        client
            .delete_bucket(bucket_name)
            .send()
            .await
            .expect("failed to delete existing bucket");
    }

    async fn delete_bucket_objects(client: &s3::Client, bucket_name: &str) {
        let mut list_objects_stream = client
            .list_objects(bucket_name)
            .recursive(true)
            .to_stream()
            .await
            .map(|object| object);

        while let Some(result) = list_objects_stream.next().await {
            let list_objects: ListObjectsResponse =
                result.expect("failed to obtain next object to delete");
            let objects_to_delete: Vec<ObjectToDelete> =
                list_objects.contents.into_iter().map(Into::into).collect();
            if !objects_to_delete.is_empty() {
                client
                    .delete_objects::<_, ObjectToDelete>(bucket_name, objects_to_delete)
                    .send()
                    .await
                    .expect("failed to delete objects");
            }
        }
    }

    async fn is_bucket_empty(client: &s3::Client, bucket_name: &str) -> bool {
        let objects = client.list_objects(bucket_name).to_stream().await;
        let bucket_size = objects.count().await;
        bucket_size == 0
    }
}
