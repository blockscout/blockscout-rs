use user_ops_indexer_proto::blockscout::user_ops_indexer::v1;

#[derive(Default, Clone)]
pub struct IndexerStatus {
    pub v06: EntryPointIndexerStatus,
    pub v07: EntryPointIndexerStatus,
}

#[derive(Default, Clone)]
pub struct EntryPointIndexerStatus {
    pub enabled: bool,
    pub live: bool,
    pub past_db_logs_indexing_finished: bool,
    pub past_rpc_logs_indexing_finished: bool,
}

impl EntryPointIndexerStatus {
    pub fn finished_past_indexing(&self) -> bool {
        !self.enabled
            || (self.past_db_logs_indexing_finished && self.past_rpc_logs_indexing_finished)
    }
}

pub struct IndexerStatusMessage {
    pub version: String,
    pub message: EntryPointIndexerStatusMessage,
}

impl From<IndexerStatus> for v1::IndexerStatus {
    fn from(status: IndexerStatus) -> Self {
        Self {
            finished_past_indexing: status.v06.finished_past_indexing()
                && status.v07.finished_past_indexing(),
            v06: Some(status.v06.into()),
            v07: Some(status.v07.into()),
        }
    }
}

impl From<EntryPointIndexerStatus> for v1::EntryPointIndexerStatus {
    fn from(status: EntryPointIndexerStatus) -> Self {
        Self {
            enabled: status.enabled,
            live: status.live,
            past_db_logs_indexing_finished: status.past_db_logs_indexing_finished,
            past_rpc_logs_indexing_finished: status.past_rpc_logs_indexing_finished,
        }
    }
}

pub enum EntryPointIndexerStatusMessage {
    IndexerStarted,
    PastDbLogsIndexingFinished,
    PastRpcLogsIndexingFinished,
}

impl IndexerStatusMessage {
    pub fn new(version: &str, message: EntryPointIndexerStatusMessage) -> Self {
        Self {
            version: version.to_string(),
            message,
        }
    }
}

impl IndexerStatusMessage {
    pub fn update_status(self, status: &mut IndexerStatus) {
        let status = match self.version.as_str() {
            "v0.6" => &mut status.v06,
            "v0.7" => &mut status.v07,
            _ => return,
        };
        self.message.update_status(status);
    }
}

impl EntryPointIndexerStatusMessage {
    pub fn update_status(self, status: &mut EntryPointIndexerStatus) {
        match self {
            EntryPointIndexerStatusMessage::IndexerStarted => {
                status.live = true;
            }
            EntryPointIndexerStatusMessage::PastDbLogsIndexingFinished => {
                status.past_db_logs_indexing_finished = true;
            }
            EntryPointIndexerStatusMessage::PastRpcLogsIndexingFinished => {
                status.past_rpc_logs_indexing_finished = true;
            }
        }
    }
}
