// Registry discovery via VerifiableFactory.ProxyDeployed and shared registry
// and root-namespace creation helpers.
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
// call site passes null, which is fine: a real deployed registry's
// ProxyDeployed event is always indexed before anything else could
// reference its address (the address can't be referenced on-chain before
// the proxy exists), so by the time any other call site runs, the row
// already exists with its real classification and this function never
// actually reruns for it.
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
    registry.discoveredAt = block.timestamp;
    registry.createdAtBlock = block.number;
    registry.updatedAtBlock = block.number;
    registry.save();
  }
  return registry;
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
    namespace.pathCount = 0;
    namespace.createdAt = block.timestamp;
    namespace.createdAtBlock = block.number;
    namespace.updatedAtBlock = block.number;
    namespace.save();

    // materializePathsForSlot enumerates namespaces strictly via
    // ENSv2RegistryNamespaceIndex (bounded by namespaceCount), so a counter
    // increment must always have a matching index row.
    let registry = ENSv2Registry.load(rootRegistryId)!;
    appendRegistryNamespaceIndex(registry, namespace);
  }
  return namespace;
}

// VerifiableFactory deploys registry, resolver, and HCA proxies. Resolver
// events are covered by the addressless PermissionedResolver source, while
// HCAs have their own entity; only registry implementations create dynamic
// registry sources and ENSv2Registry rows.
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

  ENSv2RegistryTemplate.create(event.params.proxyAddress);
  let registry = getOrCreateRegistry(
    event.params.proxyAddress,
    event.params.proxyAddress,
    event.block,
    implementation
  );
  // Also populate an existing row if another event referenced the address
  // before ProxyDeployed was processed.
  registry.implementation = implementation;
  registry.save();
}
