use sea_orm::DatabaseConnection;

pub struct ReadWriteRepo {
    write_db: DatabaseConnection,
    read_db: Option<DatabaseConnection>,
}

impl ReadWriteRepo {
    pub fn new(write_db: DatabaseConnection, read_db: Option<DatabaseConnection>) -> Self {
        Self { write_db, read_db }
    }

    pub fn write_db(&self) -> &DatabaseConnection {
        &self.write_db
    }

    pub fn read_db(&self) -> &DatabaseConnection {
        // TODO: In case read_db is not available (check `pg_last_xact_replay_timestamp`),
        // fallback to write_db
        self.read_db.as_ref().unwrap_or(&self.write_db)
    }
}
