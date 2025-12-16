# InfinityName Subgraph

This subgraph indexes events from the InfinityName protocol, a domain name service on Base network.

## Overview

InfinityName is an ERC-721 based domain name service that uses a simple hash calculation (`keccak256(domain + suffix)`) instead of ENS namehash. The protocol uses the `.blue` suffix.

## Key Differences from ENS

1. **Hash Calculation**: Uses `keccak256(domain + suffix)` instead of ENS namehash
2. **Resolved Address**: The owner of a domain is always the resolved address. When a primary domain is set, that domain's owner becomes the resolved address for the account.
3. **No Resolver Contract**: InfinityName doesn't use a separate resolver contract like ENS.

## Events Indexed

- `DomainRegistered`: Emitted when a new domain is registered
- `PrimaryDomainSet`: Emitted when an account sets a domain as their primary domain
- `PrimaryDomainReset`: Emitted when a primary domain is reset (usually on transfer)
- `Transfer`: Standard ERC-721 transfer event
- `TokenSeized`: Emitted when the contract owner seizes a token (emergency transfer)

## Schema

The subgraph follows the same schema structure as other name service subgraphs in this repository, with the following key entities:

- `Domain`: Represents a registered domain name
- `Account`: Represents a user account
- `Registration`: Represents a domain registration
- `TokenIdToDomain`: Mapping entity to efficiently look up domains by token ID

## Setup

1. Install dependencies:
```bash
yarn install
```

2. Generate code:
```bash
yarn codegen
```

3. Build:
```bash
yarn build
```

4. Deploy (update `networks.json` with the correct contract address and start block):
```bash
yarn deploy-local
```

## Configuration

Update `networks.json` with the correct contract address and start block for your deployment.

## Notes

- The contract address and start block in `subgraph.yaml` and `networks.json` are placeholders and need to be updated with actual values.
- The suffix is hardcoded as `.blue` in `utils.ts` based on the contract initialization.

