#!/usr/bin/env node
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const envPath = path.join(root, '.env');

if (fs.existsSync(envPath)) {
  const lines = fs.readFileSync(envPath, 'utf8').split(/\r?\n/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const idx = trimmed.indexOf('=');
    if (idx === -1) continue;
    const key = trimmed.slice(0, idx).trim();
    const value = trimmed.slice(idx + 1).trim();
    if (!process.env[key]) process.env[key] = value;
  }
}

const subgraphName = process.env.SUBGRAPH_NAME || 'hoodid/hoodid-bens';
const networkName = process.env.GRAPH_NODE_NETWORK_NAME || 'hood';
const versionLabel = process.env.GRAPH_NODE_VERSION_LABEL || '0.0.1';
const nodeUrl = process.env.GRAPH_NODE_ADMIN_URL || '';
const ipfsUrl = process.env.GRAPH_NODE_IPFS_URL || '';
const graphBin = process.platform === 'win32'
  ? path.join(root, 'node_modules', '.bin', 'graph.cmd')
  : path.join(root, 'node_modules', '.bin', 'graph');

function run(args, opts = {}) {
  console.log(`\n$ graph ${args.join(' ')}`);
  const result = spawnSync(graphBin, args, { cwd: root, stdio: 'inherit', ...opts });
  if (result.status !== 0) process.exit(result.status || 1);
}

if (!nodeUrl || !ipfsUrl) {
  console.error('Missing graph-node deployment config.');
  console.error('Create a .env file in this subgraph directory with:');
  console.error('SUBGRAPH_NAME=hoodid/hoodid-bens');
  console.error('GRAPH_NODE_NETWORK_NAME=hood');
  console.error('GRAPH_NODE_VERSION_LABEL=0.0.1');
  console.error('GRAPH_NODE_ADMIN_URL=<graph-node admin URL, usually http://127.0.0.1:8020>');
  console.error('GRAPH_NODE_IPFS_URL=<IPFS URL, usually http://127.0.0.1:5001>');
  process.exit(2);
}

run(['codegen']);
run(['build']);

const create = spawnSync(graphBin, ['create', subgraphName, '--node', nodeUrl], { cwd: root, stdio: 'inherit' });
if (create.status !== 0) {
  console.warn('graph create failed. Continuing to deploy in case the subgraph already exists.');
}
run(['deploy', subgraphName, '--node', nodeUrl, '--ipfs', ipfsUrl, '--network', networkName, '--version-label', versionLabel]);

console.log('\nHoodID BENS subgraph deploy command completed.');
console.log(`Subgraph name: ${subgraphName}`);
console.log(`Graph node: ${nodeUrl}`);
console.log(`Network: ${networkName}`);
console.log(`Version: ${versionLabel}`);
