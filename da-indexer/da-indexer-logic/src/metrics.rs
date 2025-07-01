use lazy_static::lazy_static;
use prometheus::{register_histogram, register_int_counter, Histogram, IntCounter};

lazy_static! {
    pub static ref S3_BULK_UPLOAD_TOTAL: IntCounter = register_int_counter!(
        "da_indexer_s3_bulk_upload_total",
        "total number of attempts to upload bulk of files into s3 storage",
    )
    .unwrap();
    pub static ref S3_BULK_UPLOAD_SUCCESSES: IntCounter = register_int_counter!(
        "da_indexer_s3_bulk_upload_successes",
        "number of successful attempts to upload bulk of file into s3 storage",
    )
    .unwrap();
    pub static ref S3_BULK_UPLOAD_TIME: Histogram = register_histogram!(
        "da_indexer_s3_bulk_upload_time_seconds",
        "bulk upload time into s3 storage in seconds",
    )
    .unwrap();
}
