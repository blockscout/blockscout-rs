#!/usr/bin/env node
// Detects address and start-block drift in two places:
//
// 1. subgraph.yaml vs networks.json for manifest data sources.
// 2. src/ensv2Constants.ts vs the ENSv2 deployment artifacts for addresses
//    that are used by mappings but are not manifest data sources.
//
// The deployment-artifact check is skipped when a sibling contracts-v2
// checkout is unavailable, so this script also works in a standalone clone.
//
// Both files use stable shapes that can be checked without adding parser
// dependencies.
//
// Usage: node scripts/validate-networks.mjs
// Exits non-zero (and prints every problem, from both checks) if anything
// is found. Missing contracts-v2 is reported informationally, not counted
// as a problem.

import { readFileSync, existsSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, "..");
const parentDir = join(repoRoot, "..");

function parseDataSources(yamlText) {
  // Matches both the `dataSources:` and `templates:` sections — both use
  // the same "- kind: ethereum/contract" block shape and indentation.
  const lines = yamlText.split("\n");
  const sources = [];
  let current = null;
  let inTemplates = false;
  for (const line of lines) {
    if (/^templates:\s*$/.test(line)) {
      inTemplates = true;
    }
    if (/^\s{2}-\s+kind:\s+ethereum\/contract\s*$/.test(line)) {
      if (current) sources.push(current);
      current = {
        name: null,
        network: null,
        address: null,
        startBlock: null,
        isTemplate: inTemplates,
      };
      continue;
    }
    if (!current) continue;
    let m;
    if ((m = line.match(/^\s{4}name:\s*(.+?)\s*$/))) {
      current.name = m[1];
    } else if ((m = line.match(/^\s{4}network:\s*(.+?)\s*$/))) {
      current.network = m[1];
    } else if ((m = line.match(/^\s{6}address:\s*"?([0-9a-fA-Fx]+)"?\s*$/))) {
      current.address = m[1].toLowerCase();
    } else if ((m = line.match(/^\s{6}startBlock:\s*(\d+)\s*$/))) {
      current.startBlock = Number(m[1]);
    }
  }
  if (current) sources.push(current);
  return sources;
}

function checkSubgraphYamlVsNetworksJson() {
  const yamlText = readFileSync(join(repoRoot, "subgraph.yaml"), "utf8");
  const networks = JSON.parse(
    readFileSync(join(repoRoot, "networks.json"), "utf8")
  );

  const sources = parseDataSources(yamlText);
  const problems = [];

  for (const src of sources) {
    if (!src.name || !src.network) continue;

    const networkEntries = networks[src.network];
    if (!networkEntries) {
      problems.push(
        `${src.name}: subgraph.yaml declares network "${src.network}", but networks.json has no "${src.network}" block at all.`
      );
      continue;
    }

    const entry = networkEntries[src.name];
    if (!entry) {
      // A template (no static address) has nothing to cross-check beyond
      // "does this network exist in networks.json" — already handled above.
      if (src.address) {
        problems.push(
          `${src.name}: subgraph.yaml hardcodes address ${src.address} under network "${src.network}", but networks.json's "${src.network}" block has no entry for "${src.name}" at all — a plain \`graph deploy --network ${src.network}\` would leave this address exactly as committed, unsubstituted.`
        );
      }
      continue;
    }

    if (src.address) {
      if (!entry.address) {
        problems.push(
          `${src.name}: subgraph.yaml has address ${src.address}, but networks.json's "${src.network}" entry has no address.`
        );
      } else if (entry.address.toLowerCase() !== src.address) {
        problems.push(
          `${src.name}: subgraph.yaml hardcodes ${src.address} for network "${src.network}", but networks.json's "${src.network}" entry says ${entry.address.toLowerCase()}.`
        );
      }
    }
    if (src.startBlock !== null) {
      if (entry.startBlock === undefined) {
        problems.push(
          `${src.name}: subgraph.yaml starts at block ${src.startBlock}, but networks.json's "${src.network}" entry has no startBlock.`
        );
      } else if (entry.startBlock !== src.startBlock) {
        problems.push(
          `${src.name}: subgraph.yaml starts at block ${src.startBlock} for network "${src.network}", but networks.json says ${entry.startBlock}.`
        );
      }
    }
  }

  console.log(
    `[1/2] subgraph.yaml vs networks.json: ${sources.length} data source(s)/template(s) checked.`
  );
  return problems;
}

function checkRequestedNetwork(requestedNetwork) {
  if (!requestedNetwork) return [];

  const yamlText = readFileSync(join(repoRoot, "subgraph.yaml"), "utf8");
  const networks = JSON.parse(
    readFileSync(join(repoRoot, "networks.json"), "utf8")
  );
  const sources = parseDataSources(yamlText);
  const target = networks[requestedNetwork];
  if (!target) {
    return [`requested deployment network "${requestedNetwork}" has no networks.json block.`];
  }

  const problems = [];
  for (const src of sources) {
    // Templates have no static chain position. Addressless data sources do,
    // and still require a target-network startBlock.
    if (!src.name || src.isTemplate) continue;
    const entry = target[src.name];
    if (!entry) {
      problems.push(
        `${src.name}: requested network "${requestedNetwork}" has no complete networks.json entry. Deployment is disabled until its real address/start block is configured.`
      );
      continue;
    }
    if (src.address !== null && !entry.address) {
      problems.push(
        `${src.name}: requested network "${requestedNetwork}" is missing its address.`
      );
    }
    if (entry.startBlock === undefined) {
      problems.push(
        `${src.name}: requested network "${requestedNetwork}" is missing its startBlock.`
      );
    }
  }
  console.log(
    `[target] ${requestedNetwork}: ${sources.length} data source(s)/template(s) checked for deployment completeness.`
  );
  return problems;
}

// Function name -> ordered deployment files. Order matters for
// getMigrationControllers, whose values are [Locked, Unlocked].
const CONSTANTS_TO_DEPLOYMENT = {
  getRootRegistryAddress: ["RootRegistry.json"],
  getEthRegistryAddress: ["ETHRegistry.json"],
  getUserRegistryImplAddress: ["UserRegistryImpl.json"],
  getWrapperRegistryImplAddress: ["WrapperRegistryImpl.json"],
  getPermissionedResolverImplAddress: ["PermissionedResolverImpl.json"],
  getStandaloneHCAImplAddress: ["StandaloneHCAImplementation.json"],
  getMigrationControllers: [
    "LockedMigrationController.json",
    "UnlockedMigrationController.json",
  ],
};

// Extract Address.fromString("0x...") literals from one network branch of
// an exported function.
function extractAddressLiterals(tsText, functionName, network) {
  const startMarker = `export function ${functionName}(`;
  const startIdx = tsText.indexOf(startMarker);
  if (startIdx === -1) return null;
  const nextExportIdx = tsText.indexOf("export function ", startIdx + startMarker.length);
  const body = tsText.slice(startIdx, nextExportIdx === -1 ? undefined : nextExportIdx);
  const branchMarker = `if (network == "${network}") {`;
  const branchStart = body.indexOf(branchMarker);
  if (branchStart === -1) return [];
  const nextBranch = body.indexOf("\n  if (network ==", branchStart + branchMarker.length);
  const branch = body.slice(branchStart, nextBranch === -1 ? undefined : nextBranch);
  const matches = [...branch.matchAll(/Address\.fromString\(\s*"(0x[0-9a-fA-F]+)"\s*\)/g)];
  return matches.map((m) => m[1].toLowerCase());
}

function checkEnsv2ConstantsVsContractsV2(requestedNetwork) {
  // Preserve the historical no-argument behavior, which checked Sepolia.
  const validationNetwork = requestedNetwork || "sepolia";
  const deploymentsDir = join(
    parentDir,
    "contracts-v2",
    "contracts",
    "deployments",
    validationNetwork
  );
  const hasDeploymentArtifacts = existsSync(deploymentsDir);

  const constantsText = readFileSync(
    join(repoRoot, "src", "ensv2Constants.ts"),
    "utf8"
  );
  const problems = [];
  let checkedCount = 0;

  for (const [functionName, deploymentFiles] of Object.entries(
    CONSTANTS_TO_DEPLOYMENT
  )) {
    const literals = extractAddressLiterals(
      constantsText,
      functionName,
      validationNetwork
    );
    if (literals === null) {
      problems.push(
        `${functionName}: expected to find this function in src/ensv2Constants.ts, but it's gone — this check's CONSTANTS_TO_DEPLOYMENT mapping is now stale and needs updating alongside whatever renamed/removed it.`
      );
      continue;
    }
    if (literals.length !== deploymentFiles.length) {
      problems.push(
        `${functionName}: found ${literals.length} Address.fromString(...) literal(s) in src/ensv2Constants.ts, but expected ${deploymentFiles.length} (one per ${deploymentFiles.join(", ")}) — this check's assumptions about this function's shape no longer hold, review by hand.`
      );
      continue;
    }

    // A standalone clone, or a network without checked-out deployment
    // artifacts (currently mainnet), can still validate that the requested
    // network branch is present and complete. Address equality is checked
    // when matching contracts-v2 artifacts are available.
    if (!hasDeploymentArtifacts) continue;

    for (let i = 0; i < deploymentFiles.length; i++) {
      const deploymentPath = join(deploymentsDir, deploymentFiles[i]);
      if (!existsSync(deploymentPath)) {
        console.log(
          `[2/2] ${functionName} / ${deploymentFiles[i]}: SKIPPED (no such deployment file in the currently-checked-out contracts-v2 commit).`
        );
        continue;
      }
      const deployment = JSON.parse(readFileSync(deploymentPath, "utf8"));
      const realAddress = (deployment.address || "").toLowerCase();
      checkedCount++;
      if (!realAddress) {
        problems.push(
          `${functionName} / ${deploymentFiles[i]}: deployment record has no "address" field at all — can't cross-check.`
        );
        continue;
      }
      if (realAddress !== literals[i]) {
        problems.push(
          `${functionName}: src/ensv2Constants.ts hardcodes ${literals[i]}, but contracts-v2/contracts/deployments/${validationNetwork}/${deploymentFiles[i]} (the currently-checked-out commit) says ${realAddress}. Either ensv2Constants.ts is stale and needs updating, or contracts-v2 has moved ahead of what this subgraph has synced to — check which before assuming.`
        );
      }
    }
  }

  console.log(
    `[2/2] src/ensv2Constants.ts (${validationNetwork}): ${checkedCount} address(es) checked${hasDeploymentArtifacts ? "." : "; contracts-v2 deployment artifacts unavailable, branch shape checked only."}`
  );
  return problems;
}

function main() {
  const networkArgIndex = process.argv.indexOf("--network");
  const requestedNetwork =
    networkArgIndex === -1 ? null : process.argv[networkArgIndex + 1];
  if (networkArgIndex !== -1 && !requestedNetwork) {
    console.error("validate-networks: --network requires a network name.");
    process.exit(2);
  }
  const problems = [
    ...(requestedNetwork
      ? checkRequestedNetwork(requestedNetwork)
      : checkSubgraphYamlVsNetworksJson()),
    ...checkEnsv2ConstantsVsContractsV2(requestedNetwork),
  ];

  if (problems.length > 0) {
    console.error(`\nvalidate-networks: ${problems.length} problem(s) found:\n`);
    for (const p of problems) console.error(`  - ${p}`);
    console.error(
      "\nResolve the configuration errors above before building or deploying the subgraph."
    );
    process.exit(1);
  }

  console.log("\nvalidate-networks: OK — no problems found.");
}

main();
