//! Update groups for Interchain mode (simple chart/counters set).

use crate::{
    construct_update_group,
    lines::interchain::{
        new_messages_interchain::NewMessagesInterchain,
        new_messages_received_interchain::NewMessagesReceivedInterchain,
        new_messages_sent_interchain::NewMessagesSentInterchain,
    },
    utils::singleton_groups,
};

use crate::counters::interchain::*;

singleton_groups!(
    TotalInterchainMessages,
    TotalInterchainMessagesReceived,
    TotalInterchainMessagesSent,
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
