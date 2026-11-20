#!/usr/bin/env node

import { copyFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const graphCli = join(
  repoRoot,
  "node_modules",
  "@graphprotocol",
  "graph-cli",
  "bin",
  "run.js"
);
const validator = join(repoRoot, "scripts", "validate-networks.mjs");
const sourceManifest = join(repoRoot, "subgraph.yaml");

function runNode(args) {
  const result = spawnSync(process.execPath, args, {
    cwd: repoRoot,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  return result.status ?? 1;
}

const [command, network, ...forwardedArgs] = process.argv.slice(2);

if (!["build", "deploy"].includes(command) || !network) {
  console.error(
    "Usage: run-network-command.mjs <build|deploy> <network> [subgraph-name] [graph-cli flags]"
  );
  process.exit(2);
}
if (!/^[a-z0-9-]+$/.test(network)) {
  console.error(`Invalid network name: ${network}`);
  process.exit(2);
}
if (command === "deploy" && (!forwardedArgs[0] || forwardedArgs[0].startsWith("-"))) {
  console.error(
    `Usage: yarn deploy:${network} <subgraph-name> [graph deploy flags]`
  );
  process.exit(2);
}

const validationStatus = runNode([validator, "--network", network]);
if (validationStatus !== 0) process.exit(validationStatus);

// graph-cli rewrites the manifest supplied with --network. Work from a
// short-lived sibling copy so relative ABI/schema/mapping paths still resolve,
// without turning subgraph.yaml into generated deployment state.
const temporaryManifest = join(
  repoRoot,
  `.subgraph.${network}.${process.pid}.yaml`
);
copyFileSync(sourceManifest, temporaryManifest);

let status = 1;
try {
  const graphArgs = [graphCli, command];
  if (command === "deploy") {
    const [subgraphName, ...deployFlags] = forwardedArgs;
    graphArgs.push(subgraphName, temporaryManifest, "--network", network, ...deployFlags);
  } else {
    graphArgs.push(temporaryManifest, "--network", network, ...forwardedArgs);
  }
  status = runNode(graphArgs);
} finally {
  rmSync(temporaryManifest, { force: true });
}

process.exit(status);
