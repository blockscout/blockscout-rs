#!/usr/bin/env python3
"""Per-stream indexing throughput from a service log.

Groups `scanning ... logs` lines by (bridge, chain, direction) and reports
batches, blocks scanned, blocks/s, and the inter-batch gap distribution. The
gap columns are the ones that matter for scheduling questions: independent
chains interleave smoothly, while chains sharing a blocked task show
near-identical gaps across every stream of one bridge.

Also counts `TimedOut` warnings, which under a shared blocking section are
manufactured rather than real -- a reqwest timeout is a tokio timer that
elapses whether or not its future is polled.

Usage:
    just run-dev > run.log 2>&1
    sed -E 's/\x1b\[[0-9;]*m//g' run.log > run.clean
    python3 scripts/indexing_throughput.py run.clean

See .memory-bank/research/indexing-concurrency-model.md for the method and for
recorded results.
"""

import re, sys
from collections import defaultdict
from datetime import datetime

if len(sys.argv) < 2:
    print(__doc__)
    sys.exit(1)

path = sys.argv[1]
ansi = re.compile(r'\x1b\[[0-9;]*m')
scan = re.compile(r'^(\S+)\s+INFO.*scanning (CATCHUP|REALTIME|RETRY)\s+logs bridge_id=(\d+) chain_id=(\d+) from_block=(\d+) to_block=(\d+)')

groups = defaultdict(list)
timeouts = 0
first = last = None
for raw in open(path, errors='replace'):
    line = ansi.sub('', raw).strip()
    if 'TimedOut' in line:
        timeouts += 1
    m = scan.match(line)
    if not m:
        continue
    ts = datetime.fromisoformat(m.group(1).replace('Z', '+00:00'))
    first = first or ts
    last = ts
    direction, bridge, chain, fb, tb = m.group(2), m.group(3), m.group(4), int(m.group(5)), int(m.group(6))
    groups[(bridge, chain, direction)].append((ts, tb - fb + 1))

span = (last - first).total_seconds() if first and last else 0
print(f"window: {span:.0f}s   scan lines: {sum(len(v) for v in groups.values())}   TimedOut warnings: {timeouts}")
print(f"{'bridge/chain/dir':<28} {'batches':>7} {'blocks':>9} {'blocks/s':>9} {'mean gap':>9} {'max gap':>8}")
for key in sorted(groups, key=lambda k: (k[0], k[1], k[2])):
    ev = groups[key]
    blocks = sum(n for _, n in ev)
    gaps = [(ev[i][0] - ev[i-1][0]).total_seconds() for i in range(1, len(ev))]
    dur = (ev[-1][0] - ev[0][0]).total_seconds()
    rate = blocks / dur if dur > 0 else float('nan')
    label = f"{key[0]}/{key[1]}/{key[2][:4]}"
    mean = sum(gaps)/len(gaps) if gaps else 0
    print(f"{label:<28} {len(ev):>7} {blocks:>9} {rate:>9.1f} {mean:>8.1f}s {max(gaps) if gaps else 0:>7.1f}s")
