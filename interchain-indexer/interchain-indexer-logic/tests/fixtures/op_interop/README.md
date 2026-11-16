# OP interop fixtures

Both files are **simulated**, not mined. They are `eth_simulateV1` results
produced on 2026-11-14 against the Lagoon contracts deployed on the OP Labs
devnets:

| File | Chain | Calls |
| --- | --- | --- |
| `simulated_send_devnet4.json` | `sepolia-devnet-4` (chain id `420130019`) | two: `SuperchainETHBridge.sendETH`, then a plain `L2ToL2CrossDomainMessenger.sendMessage` |
| `simulated_relay_devnet5.json` | `sepolia-devnet-5` (chain id `420130020`) | one: `L2ToL2CrossDomainMessenger.relayMessage` of the first message |

What is real and what is not:

- **Log topics and data are what the deployed bytecode emits.** They are the
  authority for event signatures, `topic0` values, the message hash and the
  payload hash.
- **Block hashes and transaction hashes are synthetic.** No interop message
  had been mined on either chain when the files were produced, so the
  simulation invented the block it ran in. Do not look these hashes up on a
  block explorer.

Shape: one block object with `calls[]`; each call has `logs[]`, and every log
is a standard RPC log (camelCase, with `blockHash`, `blockTimestamp` and
`logIndex`), so it deserializes straight into `alloy::rpc::types::Log`.

Besides the five events the indexer subscribes to (`SentMessage`,
`ExecutingMessage`, `RelayedMessage`, `SendETH`, `RelayETH`), the send file
contains a `LiquidityBurned` log and the relay file a `LiquidityMinted` log.
`SendETH` and `RelayETH` are read as cross-check input for the ETH transfer
derived from the message payload; `LiquidityBurned` and `LiquidityMinted`
belong to a contract the indexer does not read and remain "must be ignored"
input.

Tests build multicall and negative cases by cloning these logs and changing
`logIndex` or their order. They never hand-encode a payload with the `sol!`
types under test: a fixture built from the declaration it checks cannot catch
a wrong signature.
