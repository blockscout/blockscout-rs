// Registry discovery and shared registry and root-namespace creation helpers.
//
// A registry's own events are indexed once it has a data source
// (ENSv2Registry.indexed). The RootRegistry and ETHRegistry are static data
// sources. Every other registry gets a dynamic one from whichever of these
// comes first (startIndexingRegistry):
//   - VerifiableFactory.ProxyDeployed (handleProxyDeployed),
//   - its own first LabelRegistered, LabelReserved or ParentUpdated, seen by
//     the addressless ENSv2RegistryDiscovery source. This is what covers
//     registries that were not deployed through the factory.
// An address that is only named by another registry (as a subregistry or a
// parent) gets a row and no data source: any address can be named, at the
// cost of one event, and a data source is kept for the life of the subgraph.
import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";

import { ROOT_NODE } from "./utils";
import { appendRegistryNamespaceIndex, namespaceId } from "./ensv2Utils";
import {
  getEthRegistryAddress,
  getPermissionedResolverImplAddress,
  getRootRegistryAddress,
  getStandaloneHCAImplAddress,
  getWrapperRegistryImplAddress,
  isUserRegistryImpl,
} from "./ensv2Constants";
import { ENSv2HCA, ENSv2Namespace, ENSv2Registry } from "./types/schema";
import {
  LabelRegistered,
  LabelReserved,
  ParentUpdated,
} from "./types/RootRegistry/PermissionedRegistry";
import { ProxyDeployed } from "./types/VerifiableFactory/VerifiableFactory";
import { ENSv2Registry as ENSv2RegistryTemplate } from "./types/templates";

// Classifies purely from the address (+ implementation, for the
// template-discovered case), so it gives the same answer no matter which
// event/call site first causes a registry row to be created — RootRegistry's
// own first event, ETHRegistry's own first event, a SubregistryUpdated
// linking a child registry in, or a ParentUpdated referencing it, all agree.
// getOrCreateRegistry only sets kind once (on creation), so if any call site
// passed a hardcoded "UNKNOWN" instead of this, whichever event happened to
// create the row first would wrongly freeze it at UNKNOWN forever.
//
// `implementation` is only ever available at the one call site that has it
// (handleProxyDeployed, from the ProxyDeployed event itself) — every other
// call site passes null. A registry deployed through the factory normally
// has its row before anything else references it; one referenced first (the
// address is counterfactual) is classified when ProxyDeployed arrives, and
// one that was never deployed through the factory stays UNKNOWN.
export function kindForAddress(
  address: Address,
  implementation: Bytes | null = null
): string {
  if (address.equals(getRootRegistryAddress())) {
    return "ROOT";
  }
  if (address.equals(getEthRegistryAddress())) {
    return "ETH";
  }
  if (implementation) {
    if (isUserRegistryImpl(implementation)) {
      return "USER";
    }
    if (implementation.equals(getWrapperRegistryImplAddress())) {
      return "WRAPPER";
    }
  }
  return "UNKNOWN";
}

export function getOrCreateRegistry(
  id: Bytes,
  address: Address,
  block: ethereum.Block,
  implementation: Bytes | null = null
): ENSv2Registry {
  let registry = ENSv2Registry.load(id);
  if (registry == null) {
    registry = new ENSv2Registry(id);
    registry.address = address;
    registry.kind = kindForAddress(address, implementation);
    registry.implementation = implementation;
    registry.namespaceCount = 0;
    registry.materializedNamespaces = new Array<Bytes>();
    registry.canonicalMaterializations = 0;
    registry.aliasMaterializations = 0;
    registry.indexed = isStaticRegistry(address);
    registry.slotCount = 0;
    registry.discoveredAt = block.timestamp;
    registry.createdAtBlock = block.number;
    registry.updatedAtBlock = block.number;
    registry.save();
  }
  return registry;
}

// The RootRegistry and ETHRegistry are static data sources in the manifest.
function isStaticRegistry(address: Address): boolean {
  return (
    address.equals(getRootRegistryAddress()) ||
    address.equals(getEthRegistryAddress())
  );
}

// Makes sure the registry's own events are indexed: creates its row if it
// has none and, the first time, its dynamic data source. A data source
// created in a block also receives that block's earlier events, so the event
// that led here reaches the registry mapping too.
function startIndexingRegistry(
  address: Address,
  block: ethereum.Block,
  implementation: Bytes | null = null
): ENSv2Registry {
  let registry = getOrCreateRegistry(address, address, block, implementation);
  if (!registry.indexed) {
    ENSv2RegistryTemplate.create(address);
    registry.indexed = true;
    registry.save();
  }
  return registry;
}

// ENSv2RegistryDiscovery: an addressless data source for the events a
// registry emits before or when its first name exists. The handlers only
// make sure the emitting registry is indexed; the registry's own data source
// handles the event itself. A registry that declares its parent or registers
// names before any parent links it is picked up here, so its declaration and
// those names are known when the link arrives.
export function handleRegistryLabelRegistered(event: LabelRegistered): void {
  startIndexingRegistry(event.address, event.block);
}

export function handleRegistryLabelReserved(event: LabelReserved): void {
  startIndexingRegistry(event.address, event.block);
}

export function handleRegistryParentUpdated(event: ParentUpdated): void {
  startIndexingRegistry(event.address, event.block);
}

export function getOrCreateRootNamespace(
  rootRegistryId: Bytes,
  block: ethereum.Block
): ENSv2Namespace {
  let rootNamehash = ROOT_NODE;
  let id = namespaceId(rootRegistryId, rootNamehash);
  let namespace = ENSv2Namespace.load(id);
  if (namespace == null) {
    namespace = new ENSv2Namespace(id);
    namespace.registry = rootRegistryId;
    // Root has no name; leave the nullable field unset.
    namespace.baseNamehash = rootNamehash;
    namespace.active = true;
    namespace.materialized = true;
    namespace.backfilled = true;
    namespace.backfillCursor = 0;
    namespace.backfillQueued = false;
    namespace.canonical = true;
    namespace.pathCount = 0;
    namespace.createdAt = block.timestamp;
    namespace.createdAtBlock = block.number;
    namespace.updatedAtBlock = block.number;
    namespace.save();

    // materializePathsForSlot projects a registration under the registry's
    // materializedNamespaces; the index row keeps every namespace enumerable.
    let registry = ENSv2Registry.load(rootRegistryId)!;
    appendRegistryNamespaceIndex(registry, namespace);
    registry = ENSv2Registry.load(rootRegistryId)!;
    let materialized = registry.materializedNamespaces;
    materialized.push(namespace.id);
    registry.materializedNamespaces = materialized;
    registry.save();
  }
  return namespace;
}

// VerifiableFactory deploys registry, resolver, and HCA proxies. Resolver
// events are covered by the addressless PermissionedResolver source, while
// HCAs have their own entity; only registry implementations create dynamic
// registry sources and ENSv2Registry rows.
//
// A proxy that emitted a name or parent event before this event was
// processed is already indexed (see the file header); one that was only
// named by another registry has a row and is started here.
export function handleProxyDeployed(event: ProxyDeployed): void {
  let implementation = event.params.implementation;
  if (implementation.equals(getPermissionedResolverImplAddress())) {
    return;
  }
  if (implementation.equals(getStandaloneHCAImplAddress())) {
    // Preserve idempotence if the same deployment event is ever replayed.
    if (ENSv2HCA.load(event.params.proxyAddress) == null) {
      let hca = new ENSv2HCA(event.params.proxyAddress);
      hca.implementation = implementation;
      hca.deployer = event.params.sender;
      hca.discoveredAt = event.block.timestamp;
      hca.createdAtBlock = event.block.number;
      hca.save();
    }
    return;
  }

  let registry = startIndexingRegistry(
    event.params.proxyAddress,
    event.block,
    implementation
  );
  // Also populate an existing row if another event referenced the address
  // before ProxyDeployed was processed (a SubregistryUpdated or ParentUpdated
  // naming the counterfactual address, or a testnet redeploy). Such a row
  // was created without an implementation and
  // therefore classified UNKNOWN; now that the implementation is known,
  // classify it the same way a fresh row would be. A row that already has a
  // definite kind is left alone.
  registry.implementation = implementation;
  if (registry.kind == "UNKNOWN") {
    registry.kind = kindForAddress(event.params.proxyAddress, implementation);
  }
  registry.save();
}
