// Per-network ENSv2 constants that networks.json has no room for (it only
// covers data-source address/startBlock). Branches on dataSource.network().
//
// Unknown networks fail loudly because these values participate in entity-ID
// construction and contract classification; zero-value fallbacks would
// silently corrupt indexed state.
import { Address, BigInt, Bytes, dataSource, log } from "@graphprotocol/graph-ts";

export function getMigrationControllers(): Address[] {
  let network = dataSource.network();
  if (network == "mainnet") {
    return [
      Address.fromString("0x3cea18620883a69ed349b23328a3b6302598e51a"), // LockedMigrationController
      Address.fromString("0xab821ca676cf2e0790fa273526a165cfa86aeb64"), // UnlockedMigrationController
    ];
  }
  if (network == "sepolia") {
    return [
      Address.fromString("0x6029a063d69b09D23c52a754a90E4FE43aDac3A8"), // LockedMigrationController
      Address.fromString("0x2a35B94DF22cc7354570be2284655E2CDC0e64A2"), // UnlockedMigrationController
    ];
  }
  log.critical(
    "getMigrationControllers: no migration controller addresses configured for network '{}'. Refusing to silently return an empty list (which would make isMigrationController() always return false) — add real addresses for this network or fix the manifest's network label.",
    [network]
  );
  return [];
}

// Use .equals() for graph-ts Address comparison.
export function isMigrationController(sender: Address): boolean {
  let controllers = getMigrationControllers();
  for (let i = 0; i < controllers.length; i++) {
    if (controllers[i].equals(sender)) {
      return true;
    }
  }
  return false;
}

// The ENSv2 migration parks retired ENSv1 state (registry owner, BaseRegistrar
// token) in the Graveyard. A registry NewOwner/Transfer to it is the
// migration's v1 voiding step, not a real change of control. Returned as a
// list (empty for unknown networks) so callers never compare a nullable
// Address: Bytes overloads ==/!=, and a null comparison crashes the
// AssemblyScript compiler (compileBinaryOverload).
export function getGraveyardAddresses(): Address[] {
  let network = dataSource.network();
  if (network == "mainnet") {
    return [
      Address.fromString("0xda021a2c1c2f99574d861ef8c6fa745851beb7f6"),
    ];
  }
  if (network == "sepolia") {
    return [
      Address.fromString("0xb58a90a39d13cce1d0e192b5da5c47640855b04d"),
    ];
  }
  return [];
}

// True for the Graveyard and the migration controllers: ENSv1 ownership
// moving to one of these is part of a migration, whose legacy wrapped state
// is kept and corrected by correctMigratedLegacyOwner.
export function isMigrationDestination(address: Address): boolean {
  let graveyards = getGraveyardAddresses();
  for (let i = 0; i < graveyards.length; i++) {
    if (graveyards[i].equals(address)) {
      return true;
    }
  }
  return isMigrationController(address);
}

export function getV2GracePeriod(): BigInt {
  // GRACE_PERIOD is an ETHRegistrar *constructor argument*, not a compiled-in
  // protocol constant (contracts-v2/contracts/src/registrar/ETHRegistrar.sol)
  // — a different deployment can legitimately be constructed with a
  // different value, so this must be configured per network.
  let network = dataSource.network();
  if (network == "mainnet" || network == "sepolia") {
    // 28 days (2,419,200s) — contracts-v2/contracts/script/deploy-constants.ts
    // sets GRACE_PERIOD_V2 to this. Verified against the current Sepolia
    // ETHRenewerV1 deployment (0xf2ece44980778966b8a0fccb3a9e339440f6e045):
    // GRACE_PERIOD() returns 7776001, exactly PREMIGRATION_BONUS_PERIOD
    // (5356801) + GRACE_PERIOD_V2 (2419200).
    return BigInt.fromI32(2419200);
  }
  log.critical(
    "getV2GracePeriod: no grace period configured for network '{}'. GRACE_PERIOD is a per-deployment constructor argument — confirm the real on-chain value for this network's ETHRegistrar/ETHRenewerV1 (e.g. via GRACE_PERIOD()) before adding it here; do not assume the Sepolia value applies.",
    [network]
  );
  return BigInt.zero();
}

// The mapping runtime does not expose a data-source name through the
// `dataSource` host API, so RootRegistry/ETHRegistry can't be told apart from
// a template-discovered registry by data-source name — compare event.address
// against these instead. Values are network-specific.
export function getRootRegistryAddress(): Address {
  let network = dataSource.network();
  if (network == "mainnet") {
    return Address.fromString("0x2553c09be6447beffdb86fa622ad41fb8f77a91e");
  }
  if (network == "sepolia") {
    return Address.fromString("0xB458D6a3a77919449d03e7A6903C26827c1eC43f");
  }
  log.critical(
    "getRootRegistryAddress: no RootRegistry address configured for network '{}'. Refusing to silently return the zero address (which would collapse every registry-id/kind lookup onto one bogus bucket) — add the real address for this network or fix the manifest's network label.",
    [network]
  );
  return Address.zero();
}

export function getEthRegistryAddress(): Address {
  let network = dataSource.network();
  if (network == "mainnet") {
    return Address.fromString("0xf60aba43e1f7f3dab1186ff03b561401c9df85e2");
  }
  if (network == "sepolia") {
    return Address.fromString("0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4");
  }
  log.critical(
    "getEthRegistryAddress: no ETHRegistry address configured for network '{}'. Refusing to silently return the zero address (which would collapse every ENSv2Registration id onto one bogus bucket) — add the real address for this network or fix the manifest's network label.",
    [network]
  );
  return Address.zero();
}

// Implementation (not proxy) addresses behind VerifiableFactory.ProxyDeployed
// — the signal kindForAddress uses to classify a template-discovered
// registry as USER/WRAPPER, and to tell a resolver deployment apart from a
// registry one. Values are configured from the deployment artifacts for each
// network. Address comparison is byte-level, so checksum casing has no effect.
export function getUserRegistryImplAddress(): Address {
  let network = dataSource.network();
  if (network == "mainnet") {
    return Address.fromString("0xfaa23592aca10a6d8875b18ea86954e8eb6d8d72");
  }
  if (network == "sepolia") {
    return Address.fromString("0x9BD8a88719068D09ecee662f36C0E3856708366a");
  }
  log.critical(
    "getUserRegistryImplAddress: no UserRegistry implementation address configured for network '{}'. Refusing to silently return the zero address (which would misclassify every USER registry as UNKNOWN) — add the real address for this network or fix the manifest's network label.",
    [network]
  );
  return Address.zero();
}

// Further implementations that VerifiableFactory proxies may point at and
// that must still classify as USER. The canonical artifact address above is
// what `yarn validate` checks; these are implementations observed on-chain
// that the deployment records do not (yet) list. Mainnet: the ENS app and
// ENSjs deploy user registries against 0xC927bCaA… (first seen at block
// 26288738, 286 proxies by 2026-10-30), while namechain's addresses.md still
// records 0xfaa23592… (960 proxies). Both are live, so both count.
export function getAdditionalUserRegistryImplAddresses(): Address[] {
  let network = dataSource.network();
  if (network == "mainnet") {
    return [
      Address.fromString("0xC927bCaA77cd7bE283518760896Be0CF3Cfbe08d"), // UserRegistryImpl used by the ENS app / ENSjs
    ];
  }
  return [];
}

export function isUserRegistryImpl(implementation: Bytes): boolean {
  if (implementation.equals(getUserRegistryImplAddress())) {
    return true;
  }
  let extra = getAdditionalUserRegistryImplAddresses();
  for (let i = 0; i < extra.length; i++) {
    if (implementation.equals(extra[i])) {
      return true;
    }
  }
  return false;
}

export function getWrapperRegistryImplAddress(): Address {
  let network = dataSource.network();
  if (network == "mainnet") {
    return Address.fromString("0xe86a4ef4d7a210aaa1d6a9594fc7add7cd2cf888");
  }
  if (network == "sepolia") {
    return Address.fromString("0xBe768b63E5fBBFBB0Ae97E9064E0002dF8001880");
  }
  log.critical(
    "getWrapperRegistryImplAddress: no WrapperRegistry implementation address configured for network '{}'. Refusing to silently return the zero address (which would misclassify every WRAPPER registry as UNKNOWN) — add the real address for this network or fix the manifest's network label.",
    [network]
  );
  return Address.zero();
}

export function getPermissionedResolverImplAddress(): Address {
  let network = dataSource.network();
  if (network == "mainnet") {
    return Address.fromString("0x565b0f456a04ac9b48758190e90030623cff57c5");
  }
  if (network == "sepolia") {
    return Address.fromString("0x115eb53F0c60696633855F90b138178Fb40b2b2C");
  }
  log.critical(
    "getPermissionedResolverImplAddress: no PermissionedResolver implementation address configured for network '{}'. Refusing to silently return the zero address (which would fail to recognize resolver ProxyDeployed events, creating a bogus ENSv2Registry row for each one) — add the real address for this network or fix the manifest's network label.",
    [network]
  );
  return Address.zero();
}

// StandaloneHCAFactory (HCA = Hierarchical Context Authority, an optional
// ENS execution account — see contracts-v2/docs/HCA.md) shares this
// deployment's VerifiableFactory instance rather than deploying its own, so
// every HCA proxy fires the identical ProxyDeployed event a registry
// deployment does. Without recognizing this implementation, kindForAddress
// falls through to UNKNOWN and handleProxyDeployed wrongly creates an
// ENSv2Registry row for it; an HCA is not a registry.
export function getStandaloneHCAImplAddress(): Address {
  let network = dataSource.network();
  if (network == "mainnet") {
    return Address.fromString("0x116f462fa078ec0009e9b25dcd60309e70aac4f4");
  }
  if (network == "sepolia") {
    return Address.fromString("0xC940e5C5bF263C0e097054AECf73826769A72CEE");
  }
  log.critical(
    "getStandaloneHCAImplAddress: no StandaloneHCA implementation address configured for network '{}'. Refusing to silently return the zero address (which would fail to recognize HCA ProxyDeployed events, creating a bogus ENSv2Registry row for each one) — add the real address for this network or fix the manifest's network label.",
    [network]
  );
  return Address.zero();
}
