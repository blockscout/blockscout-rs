//! Update groups for Interchain mode (simple chart/counters set).

use crate::{
    construct_update_group,
    lines::interchain::{
        new_messages_interchain::NewMessagesInterchain,
        new_messages_received_interchain::NewMessagesReceivedInterchain,
        new_messages_sent_interchain::NewMessagesSentInterchain,
        new_transfers_interchain::NewTransfersInterchain,
        new_transfers_received_interchain::NewTransfersReceivedInterchain,
        new_transfers_sent_interchain::NewTransfersSentInterchain,
    },
    utils::singleton_groups,
};

use crate::counters::interchain::*;

singleton_groups!(
    TotalInterchainMessages,
    TotalInterchainMessagesReceived,
    TotalInterchainMessagesSent,
    TotalInterchainTransfers,
    TotalInterchainTransfersReceived,
    TotalInterchainTransfersSent,
);

construct_update_group!(NewMessagesInterchainGroup {
    charts: [NewMessagesInterchain,],
});

construct_update_group!(NewMessagesSentInterchainGroup {
    charts: [NewMessagesSentInterchain,],
});

construct_update_group!(NewMessagesReceivedInterchainGroup {
    charts: [NewMessagesReceivedInterchain,],
});

construct_update_group!(NewTransfersInterchainGroup {
    charts: [NewTransfersInterchain,],
});

construct_update_group!(NewTransfersSentInterchainGroup {
    charts: [NewTransfersSentInterchain,],
});

construct_update_group!(NewTransfersReceivedInterchainGroup {
    charts: [NewTransfersReceivedInterchain,],
});
