CREATE
OR REPLACE FUNCTION _job_queue_set_modified_at()
    RETURNS TRIGGER AS
$$
BEGIN
    NEW.modified_at
= now();
RETURN NEW;
END;
$$
LANGUAGE plpgsql;

CREATE TYPE "_job_status" AS ENUM (
    'waiting',
    'in_process',
    'success',
    'error'
    );

CREATE TABLE "_job_queue"
(
    "id"          bigserial PRIMARY KEY,

    "created_at"  timestamp   NOT NULL DEFAULT (now()),
    "modified_at" timestamp   NOT NULL DEFAULT (now()),

    "status"      _job_status NOT NULL DEFAULT 'waiting',
    "log"         varchar
);

CREATE INDEX _job_queue_status_index ON _job_queue (status);

CREATE TRIGGER "set_modified_at"
    BEFORE UPDATE
    ON "_job_queue"
    FOR EACH ROW
    EXECUTE FUNCTION _job_queue_set_modified_at();

CREATE
OR REPLACE FUNCTION _insert_job()
    RETURNS TRIGGER AS
$$
BEGIN
-- Insert a new row into the jobs table and get the ID
INSERT INTO _job_queue DEFAULT
VALUES
    RETURNING id
INTO NEW._job_id;

-- Update the jobs_id in the contract_addresses table
RETURN NEW;
END;
$$
LANGUAGE plpgsql;