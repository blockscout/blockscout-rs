// Shared EACRolesChanged handling — identical event
// signature on PermissionedRegistry and PermissionedResolver, but distinct
// generated TypeScript classes (different codegen paths) and AssemblyScript
// has no union types, so this takes primitives rather
// than either event class; ensv2Registry.ts and ensv2Resolver.ts's
// handleEACRolesChanged are both thin wrappers over this.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import { concat, createOrLoadAccount, uint256ToByteArray } from "./utils";
import { resourceId } from "./ensv2Utils";
import {
  ENSv2ResolverResourceArgument,
  ENSv2Resource,
  ENSv2RoleAssignment,
  ENSv2RoleChange,
} from "./types/schema";

export function processEACRolesChanged(
  contract: Address,
  resource: BigInt,
  account: Address,
  oldRoleBitmap: BigInt,
  newRoleBitmap: BigInt,
  block: ethereum.Block,
  transactionID: Bytes,
  logIndex: BigInt
): void {
  // A registry-emitted EACRolesChanged is dispatched here TWICE for the
  // same physical log: once via the address-bound registry data source,
  // once via the addressless PermissionedResolver wildcard source, since
  // both bind the byte-identical event signature.
  // historyId is a deterministic function of the log itself (block +
  // logIndex), so checking for it first is a free, exact way to detect and
  // skip the second dispatch — no address-based source-type check needed.
  let historyId = Bytes.fromByteArray(
    concat(uint256ToByteArray(block.number), uint256ToByteArray(logIndex))
  );
  if (ENSv2RoleChange.load(historyId) != null) {
    return;
  }

  let contractId: Bytes = contract;
  let accountEntity = createOrLoadAccount(account);

  // Fixed-width concatenation needs no delimiter. Account IDs are strings,
  // so convert the account back to its 20-byte address representation.
  let id = Bytes.fromByteArray(
    concat(
      concat(contractId, uint256ToByteArray(resource)),
      Bytes.fromHexString(accountEntity.id)
    )
  );
  let assignment = ENSv2RoleAssignment.load(id);
  if (assignment == null) {
    assignment = new ENSv2RoleAssignment(id);
    assignment.contract = contract;
    assignment.resource = resource;
    assignment.account = accountEntity.id;
  }
  // ENSv2Resource is always registry-address-prefixed, so this
  // lookup naturally (and correctly) finds nothing for a resolver-sourced
  // event, no need to know or check which kind of contract this is.
  let resourceEntity = ENSv2Resource.load(resourceId(contractId, resource));
  if (resourceEntity != null) {
    assignment.resourceEntity = resourceEntity.id;
  }
  let resourceArgument = ENSv2ResolverResourceArgument.load(
    resourceId(contractId, resource)
  );
  if (resourceArgument != null) {
    assignment.resourceArgument = resourceArgument.id;
  }
  assignment.roleBitmap = newRoleBitmap;
  assignment.updatedAtBlock = block.number;
  assignment.save();

  // historyId computed once, above (also serves as this function's own
  // dedup check for the double-dispatch case).
  let history = new ENSv2RoleChange(historyId);
  history.contract = contract;
  history.resource = resource;
  history.account = accountEntity.id;
  history.oldRoleBitmap = oldRoleBitmap;
  history.newRoleBitmap = newRoleBitmap;
  history.blockNumber = block.number;
  history.transactionID = transactionID;
  history.logIndex = logIndex;
  history.save();
}
