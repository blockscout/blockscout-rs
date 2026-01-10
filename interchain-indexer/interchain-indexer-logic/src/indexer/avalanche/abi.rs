use alloy::sol;
use serde::{Deserialize, Serialize};

sol! {
    #[derive(Debug, Deserialize, Serialize)]
    struct TeleporterMessageReceipt {
        uint256 receivedMessageNonce;
        address relayerRewardAddress;
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct TeleporterFeeInfo {
        address feeTokenAddress;
        uint256 amount;
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct TeleporterMessage {
        uint256 messageNonce;
        address originSenderAddress;
        bytes32 destinationBlockchainID;
        address destinationAddress;
        uint256 requiredGasLimit;
        address[] allowedRelayerAddresses;
        TeleporterMessageReceipt[] receipts;
        bytes message;
    }

    interface ITeleporterMessenger {
        #[derive(Debug, Deserialize, Serialize)]
        event SendCrossChainMessage(
            bytes32 indexed messageID,
            bytes32 indexed destinationBlockchainID,
            TeleporterMessage message,
            TeleporterFeeInfo feeInfo
        );

        #[derive(Debug, Deserialize, Serialize)]
        event ReceiveCrossChainMessage(
            bytes32 indexed messageID,
            bytes32 indexed sourceBlockchainID,
            address indexed deliverer,
            address rewardRedeemer,
            TeleporterMessage message
        );

        #[derive(Debug, Deserialize, Serialize)]
        event MessageExecuted(bytes32 indexed messageID, bytes32 indexed sourceBlockchainID);

        #[derive(Debug, Deserialize, Serialize)]
        event MessageExecutionFailed(
            bytes32 indexed messageID, bytes32 indexed sourceBlockchainID, TeleporterMessage message
        );
    }

    /// @notice Input parameters for transferring tokens to another chain as
    /// part of a simple transfer.
    ///
    /// @param destinationBlockchainID Blockchain ID of the destination
    ///
    /// @param destinationTokenTransferrerAddress Address of the destination
    /// token transferrer instance
    ///
    /// @param recipient Address of the recipient on the destination chain
    ///
    /// @param primaryFeeTokenAddress Address of the ERC20 contract to
    /// optionally pay a Teleporter message fee
    ///
    /// @param primaryFee Amount of tokens to pay as the optional Teleporter
    /// message fee
    ///
    /// @param secondaryFee Amount of tokens to pay for Teleporter fee if a
    /// multi-hop is needed
    ///
    /// @param requiredGasLimit Gas limit requirement for sending to a token
    /// transferrer. This is required because the gas requirement varies based
    /// on the token transferrer instance specified by
    /// {destinationBlockchainID} and {destinationTokenTransferrerAddress}.
    ///
    ///
    /// @param multiHopFallback In the case of a multi-hop transfer, the
    /// address where the tokens are sent on the home chain if the transfer is
    /// unable to be routed to its final destination. Note that this address
    /// must be able to receive the tokens held as collateral in the home
    /// contract.
    #[derive(Debug, Serialize, Deserialize)]
    struct SendTokensInput {
        bytes32 destinationBlockchainID;
        address destinationTokenTransferrerAddress;
        address recipient;
        address primaryFeeTokenAddress;
        uint256 primaryFee;
        uint256 secondaryFee;
        uint256 requiredGasLimit;
        address multiHopFallback;
    }


    /// @notice Input parameters for transferring tokens to another chain as
    /// part of a transfer with a contract call.
    ///
    /// @param destinationBlockchainID BlockchainID of the destination
    ///
    /// @param destinationTokenTransferrerAddress Address of the destination
    /// token transferrer instance
    ///
    /// @param recipientContract The contract on the destination chain that
    /// will be called
    ///
    /// @param recipientPayload The payload that will be provided to the
    /// recipient contract on the destination chain
    ///
    /// @param requiredGasLimit The required amount of gas needed to deliver
    /// the message on its destination chain, including token operations and
    /// the call to the recipient contract.
    ///
    /// @param recipientGasLimit The amount of gas that will provided to the
    /// recipient contract on the destination chain, which must be less than
    /// the requiredGasLimit of the message as a whole.
    ///
    /// @param multiHopFallback In the case of a multi-hop transfer, the
    /// address where the tokens are sent on the home chain if the transfer is
    /// unable to be routed to its final destination. Note that this address
    /// must be able to receive the tokens held as collateral in the home
    /// contract.
    ///
    /// @param fallbackRecipient Address on the {destinationBlockchainID} where
    /// the transferred tokens are sent to if the call to the recipient
    /// contract fails. Note that this address must be able to receive the
    /// tokens on the destination chain of the transfer.
    ///
    /// @param primaryFeeTokenAddress Address of the ERC20 contract to
    /// optionally pay a Teleporter message fee
    ///
    /// @param primaryFee Amount of tokens to pay for Teleporter fee on the
    /// chain that iniiated the transfer
    ///
    /// @param secondaryFee Amount of tokens to pay for Teleporter fee if a
    /// multi-hop is needed
    #[derive(Debug, Serialize, Deserialize)]
    struct SendAndCallInput {
        bytes32 destinationBlockchainID;
        address destinationTokenTransferrerAddress;
        address recipientContract;
        bytes recipientPayload;
        uint256 requiredGasLimit;
        uint256 recipientGasLimit;
        address multiHopFallback;
        address fallbackRecipient;
        address primaryFeeTokenAddress;
        uint256 primaryFee;
        uint256 secondaryFee;
    }

     /// @notice Interface for an Avalanche interchain token transferrer that
     /// sends tokens to another chain.
     ///
     /// @custom:security-contact
     /// https://github.com/ava-labs/icm-contracts/blob/main/SECURITY.md
    interface ITokenTransferrer is ITeleporterReceiver {
        /// @notice Emitted when tokens are sent to another chain.
        #[derive(Debug, Serialize, Deserialize)]
        event TokensSent(
            bytes32 indexed teleporterMessageID,
            address indexed sender,
            SendTokensInput input,
            uint256 amount
        );

        /// @notice Emitted when tokens are sent to another chain with calldata
        /// for a contract recipient.
        #[derive(Debug, Serialize, Deserialize)]
        event TokensAndCallSent(
            bytes32 indexed teleporterMessageID,
            address indexed sender,
            SendAndCallInput input,
            uint256 amount
        );

        /// @notice Emitted when tokens are withdrawn from the token transferrer
        /// contract.
        #[derive(Debug, Serialize, Deserialize)]
        event TokensWithdrawn(address indexed recipient, uint256 amount);

        /// @notice Emitted when a call to a recipient contract to receive token
        /// succeeds.
        #[derive(Debug, Serialize, Deserialize)]
        event CallSucceeded(address indexed recipientContract, uint256 amount);

        /// @notice Emitted when a call to a recipient contract to receive token
        /// fails, and the tokens are sent to a fallback recipient.
        #[derive(Debug, Serialize, Deserialize)]
        event CallFailed(address indexed recipientContract, uint256 amount);
    }

    interface ITokenHome is ITokenTransferrer {
        /// @notice Emitted when tokens are routed from a multi-hop send message
        /// to another chain.
        #[derive(Debug, Serialize, Deserialize)]
        event TokensRouted(bytes32 indexed teleporterMessageID, SendTokensInput input, uint256 amount);

        /// @notice Emitted when tokens are routed from a mulit-hop send
        /// message, with calldata for a contract recipient, to another chain.
        #[derive(Debug, Serialize, Deserialize)]
        event TokensAndCallRouted(
            bytes32 indexed teleporterMessageID, SendAndCallInput input, uint256 amount
        );
    }
}
