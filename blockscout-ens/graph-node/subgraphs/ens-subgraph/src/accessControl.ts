// Shared access-control handlers. The contracts expose separate generated
// event classes and disagree on the ApprovalForAll owner parameter name, so
// callers pass primitive values rather than generated event objects.
import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";
import { concat, createOrLoadAccount } from "./utils";
import { ContractOwnership, OperatorApproval, RegistrarController } from "./types/schema";

// No delimiter is needed because every component is a 20-byte address.
export function processApprovalForAll(
  contract: Address,
  owner: Address,
  operator: Address,
  approved: boolean,
  block: ethereum.Block
): void {
  let ownerAccount = createOrLoadAccount(owner);
  let operatorAccount = createOrLoadAccount(operator);
  // Account IDs are strings; convert them back to fixed-width bytes for the
  // composite OperatorApproval ID.
  let id = Bytes.fromByteArray(
    concat(
      concat(contract, Bytes.fromHexString(ownerAccount.id)),
      Bytes.fromHexString(operatorAccount.id)
    )
  );

  let entity = OperatorApproval.load(id);
  if (entity == null) {
    entity = new OperatorApproval(id);
    entity.contract = contract;
    entity.owner = ownerAccount.id;
    entity.operator = operatorAccount.id;
  }
  entity.approved = approved;
  entity.updatedAtBlock = block.number;
  entity.save();
}

export function processOwnershipTransferred(
  contract: Address,
  newOwner: Address,
  block: ethereum.Block
): void {
  let ownerAccount = createOrLoadAccount(newOwner);
  let id = contract;

  let entity = ContractOwnership.load(id);
  if (entity == null) {
    entity = new ContractOwnership(id);
    entity.contract = contract;
  }
  entity.owner = ownerAccount.id;
  entity.updatedAtBlock = block.number;
  entity.save();
}

export function processControllerStatus(
  contract: Address,
  controller: Address,
  active: boolean,
  block: ethereum.Block
): void {
  let id = Bytes.fromByteArray(concat(contract, controller));

  let entity = RegistrarController.load(id);
  if (entity == null) {
    entity = new RegistrarController(id);
    entity.contract = contract;
    entity.controller = controller;
  }
  entity.active = active;
  entity.updatedAtBlock = block.number;
  entity.save();
}
