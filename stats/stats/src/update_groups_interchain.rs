//! Update groups for Interchain mode (simple chart/counters set).

use crate::{
    construct_update_group,
    lines::interchain::{
        messages_growth_received_interchain::{
            MessagesGrowthReceivedInterchain, MessagesGrowthReceivedInterchainMonthly,
            MessagesGrowthReceivedInterchainWeekly, MessagesGrowthReceivedInterchainYearly,
        },
        messages_growth_sent_interchain::{
            MessagesGrowthSentInterchain, MessagesGrowthSentInterchainMonthly,
            MessagesGrowthSentInterchainWeekly, MessagesGrowthSentInterchainYearly,
        },
        new_messages_interchain::{NewMessagesInterchain, NewMessagesInterchainMonthly, NewMessagesInterchainWeekly, NewMessagesInterchainYearly},
        new_messages_received_interchain::{NewMessagesReceivedInterchain, NewMessagesReceivedInterchainMonthly, NewMessagesReceivedInterchainWeekly, NewMessagesReceivedInterchainYearly},
        new_messages_sent_interchain::{NewMessagesSentInterchain, NewMessagesSentInterchainMonthly, NewMessagesSentInterchainWeekly, NewMessagesSentInterchainYearly},
        new_transfers_interchain::{NewTransfersInterchain, NewTransfersInterchainMonthly, NewTransfersInterchainWeekly, NewTransfersInterchainYearly},
        new_transfers_received_interchain::{NewTransfersReceivedInterchain, NewTransfersReceivedInterchainMonthly, NewTransfersReceivedInterchainWeekly, NewTransfersReceivedInterchainYearly},
        new_transfers_sent_interchain::{NewTransfersSentInterchain, NewTransfersSentInterchainMonthly, NewTransfersSentInterchainWeekly, NewTransfersSentInterchainYearly},
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
    TotalInterchainTransferUsers,
);

construct_update_group!(NewMessagesInterchainGroup {
    charts: [
        NewMessagesInterchain,
        NewMessagesInterchainWeekly,
        NewMessagesInterchainMonthly,
        NewMessagesInterchainYearly,
    ],
});

construct_update_group!(NewMessagesSentInterchainGroup {
    charts: [
        NewMessagesSentInterchain,
        NewMessagesSentInterchainWeekly,
        NewMessagesSentInterchainMonthly,
        NewMessagesSentInterchainYearly,
        MessagesGrowthSentInterchain,
        MessagesGrowthSentInterchainWeekly,
        MessagesGrowthSentInterchainMonthly,
        MessagesGrowthSentInterchainYearly,
    ],
});

construct_update_group!(NewMessagesReceivedInterchainGroup {
    charts: [
        NewMessagesReceivedInterchain,
        NewMessagesReceivedInterchainWeekly,
        NewMessagesReceivedInterchainMonthly,
        NewMessagesReceivedInterchainYearly,
        MessagesGrowthReceivedInterchain,
        MessagesGrowthReceivedInterchainWeekly,
        MessagesGrowthReceivedInterchainMonthly,
        MessagesGrowthReceivedInterchainYearly,
    ],
});

construct_update_group!(NewTransfersInterchainGroup {
    charts: [
        NewTransfersInterchain,
        NewTransfersInterchainWeekly,
        NewTransfersInterchainMonthly,
        NewTransfersInterchainYearly,
    ],
});

construct_update_group!(NewTransfersSentInterchainGroup {
    charts: [
        NewTransfersSentInterchain,
        NewTransfersSentInterchainWeekly,
        NewTransfersSentInterchainMonthly,
        NewTransfersSentInterchainYearly,
    ],
});

construct_update_group!(NewTransfersReceivedInterchainGroup {
    charts: [
        NewTransfersReceivedInterchain,
        NewTransfersReceivedInterchainWeekly,
        NewTransfersReceivedInterchainMonthly,
        NewTransfersReceivedInterchainYearly,
    ],
});
