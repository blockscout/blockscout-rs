use crate::sea_orm_active_enums::{CctxStatusStatus, CoinType, ConfirmationMode, InboundStatus, Kind, ProtocolContractVersion, TxFinalizationStatus, ProcessingStatus};

impl TryFrom<String> for TxFinalizationStatus {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "NotFinalized" => Ok(TxFinalizationStatus::NotFinalized),
            "Finalized" => Ok(TxFinalizationStatus::Finalized),
            "Executed" => Ok(TxFinalizationStatus::Executed),
            _ => Err(format!("Invalid TxFinalizationStatus: {}", value)),
        }
    }
}

impl TryFrom<String> for Kind {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "realtime" => Ok(Kind::Realtime),
            "historical" => Ok(Kind::Historical),
            _ => Err(format!("Invalid Kind: {}", value)),
        }
    }
} 

impl TryFrom<String> for CctxStatusStatus {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "PendingInbound" => Ok(CctxStatusStatus::PendingInbound),
            "PendingOutbound" => Ok(CctxStatusStatus::PendingOutbound),
            "PendingRevert" => Ok(CctxStatusStatus::PendingRevert),
            "Aborted" => Ok(CctxStatusStatus::Aborted),
            "Reverted" => Ok(CctxStatusStatus::Reverted),
            "OutboundMined" => Ok(CctxStatusStatus::OutboundMined),
            _ => Err(format!("Invalid CctxStatusStatus: {}", value)),
        }
    }
}

//convert CCtxStatusStatus to i32
// enum CctxStatus {
//     PendingInbound = 0;  // some observer sees inbound tx
//     PendingOutbound = 1; // super majority observer see inbound tx
//     OutboundMined = 3;   // the corresponding outbound tx is mined
//     PendingRevert = 4;   // outbound cannot succeed; should revert inbound
//     Reverted = 5;        // inbound reverted.
//     Aborted =
//         6; // inbound tx error or invalid paramters and cannot revert; just abort.
//            // But the amount can be refunded to zetachain using and admin proposal
//   }
impl From<CctxStatusStatus> for i32 {
    fn from(status: CctxStatusStatus) -> Self {
        match status {
            CctxStatusStatus::PendingInbound => 0,
            CctxStatusStatus::PendingOutbound => 1,
            CctxStatusStatus::OutboundMined => 3,
            CctxStatusStatus::PendingRevert => 4,
            CctxStatusStatus::Reverted => 5,
            CctxStatusStatus::Aborted => 6,
        }
    }
}

//convert TxFinalizationStatus to i32
// enum TxFinalizationStatus {
//     NotFinalized = 0; // the corresponding tx is not finalized
//     Finalized = 1;    // the corresponding tx is finalized but not executed yet
//     Executed = 2;     // the corresponding tx is executed
//   }
impl From<TxFinalizationStatus> for i32 {
    fn from(status: TxFinalizationStatus) -> Self {
        match status {
            TxFinalizationStatus::NotFinalized => 0,
            TxFinalizationStatus::Finalized => 1,
            TxFinalizationStatus::Executed => 2,
        }
    }
}
//convert InboundStatus to i32
// enum InboundStatus {
//     SUCCESS = 0;
//     // this field is specifically for Bitcoin when the deposit amount is less than
//     // depositor fee
//     INSUFFICIENT_DEPOSITOR_FEE = 1;
//     // the receiver address parsed from the inbound is invalid
//     INVALID_RECEIVER_ADDRESS = 2;
//     // parse memo is invalid
//     INVALID_MEMO = 3;
//   }
impl From<InboundStatus> for i32 {
    fn from(status: InboundStatus) -> Self {
        match status {
            InboundStatus::Success => 0,
            InboundStatus::InsufficientDepositorFee => 1,
            InboundStatus::InvalidReceiverAddress => 2,
            InboundStatus::InvalidMemo => 3,
        }
    }
}

impl TryFrom<String> for InboundStatus {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "SUCCESS" => Ok(InboundStatus::Success),
            "INSUFFICIENT_DEPOSITOR_FEE" => Ok(InboundStatus::InsufficientDepositorFee),
            "INVALID_RECEIVER_ADDRESS" => Ok(InboundStatus::InvalidReceiverAddress),
            "INVALID_MEMO" => Ok(InboundStatus::InvalidMemo),
            _ => Err(format!("Invalid InboundStatus: {}", value)),
        }
    }
}

//convert ConfirmationMode to i32
// enum ConfirmationMode {
//     SAFE = 0; // an inbound/outbound is confirmed using safe confirmation count
//     FAST = 1; // an inbound/outbound is confirmed using fast confirmation count
//   }
impl From<ConfirmationMode> for i32 {
    fn from(status: ConfirmationMode) -> Self {
        match status {
            ConfirmationMode::Safe => 0,
            ConfirmationMode::Fast => 1,
        }
    }
}

impl TryFrom<String> for ConfirmationMode {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "SAFE" => Ok(ConfirmationMode::Safe),
            "FAST" => Ok(ConfirmationMode::Fast),
            _ => Err(format!("Invalid ConfirmationMode: {}", value)),
        }
    }
}
//convert CoinType to i32
// enum CoinType {
//     Zeta = 0;
//     Gas = 1;
//     ERC20 = 2;
impl From<CoinType> for i32 {
    fn from(status: CoinType) -> Self {
        match status {
            CoinType::Zeta => 0,
            CoinType::Gas => 1,
            CoinType::Erc20 => 2,
            CoinType::Cmd => 3,
            CoinType::NoAssetCall => 4,
        }
    }
}

impl TryFrom<String> for CoinType {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Zeta" => Ok(CoinType::Zeta),
            "Gas" => Ok(CoinType::Gas),
            "ERC20" => Ok(CoinType::Erc20),
            "Cmd" => Ok(CoinType::Cmd),
            "NoAssetCall" => Ok(CoinType::NoAssetCall),
            _ => Err(format!("Invalid CoinType: {}", value)),
        }
    }
}
//convert ProtocolContractVersion to i32
// enum ProtocolContractVersion {
//     V1 = 0;
//     V2 = 1;
//   }
impl From<ProtocolContractVersion> for i32 {
    fn from(status: ProtocolContractVersion) -> Self {
        match status {
            ProtocolContractVersion::V1 => 0,
            ProtocolContractVersion::V2 => 1,
        }
    }
}
impl TryFrom<String> for ProtocolContractVersion {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "V1" => Ok(ProtocolContractVersion::V1),
            "V2" => Ok(ProtocolContractVersion::V2),
            _ => Err(format!("Invalid ProtocolContractVersion: {}", value)),
        }
    }
}

impl TryFrom<String> for ProcessingStatus {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "locked" => Ok(ProcessingStatus::Locked),
            "unlocked" => Ok(ProcessingStatus::Unlocked),
            "failed" => Ok(ProcessingStatus::Failed),
            _ => Err(format!("Invalid ProcessingStatus: {}", value)),
        }
    }
}