DROP FUNCTION _insert_job;
DROP TRIGGER set_modified_at ON _job_queue;
DROP INDEX _job_queue_status_index;
DROP TABLE _job_queue;
DROP TYPE _job_status;
DROP FUNCTION _job_queue_set_modified_at;
