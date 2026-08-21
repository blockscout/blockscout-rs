#!/usr/bin/env python3
"""Measure whether one chain's activity lengthens another chain's gaps.

The decisive check for "does a slow chain drag its siblings down".

The mechanism under test: a chain throttled to a low `max_rps` spends seconds
inside `process_batch`. If the bridge's chains share one driver loop, that time
lands as a *gap* in every sibling's timeline. If they are decoupled, a sibling's
gaps are the same length whether or not the slow chain was working.

So: split each fast chain's inter-batch gaps into those that overlap a slow
chain's batch and those that do not, and compare. Coupled => the overlapping
gaps are systematically longer. Decoupled => the two distributions match.

Usage: coupling.py <log> <slow_chain_id> [fast_chain_id ...]
"""
import re
import sys
from collections import defaultdict
from datetime import datetime
from statistics import median

ANSI = re.compile(r"\x1b\[[0-9;]*m")
SCAN = re.compile(
    r"^(\S+)\s+INFO.*scanning (CATCHUP|REALTIME|RETRY)\s+logs "
    r"bridge_id=(\d+) chain_id=(\d+) from_block=(\d+) to_block=(\d+)"
)


def parse(path):
    events = defaultdict(list)
    for raw in open(path, errors="replace"):
        m = SCAN.match(ANSI.sub("", raw).strip())
        if not m:
            continue
        ts = datetime.fromisoformat(m.group(1).replace("Z", "+00:00"))
        events[m.group(4)].append(ts)
    return {c: sorted(v) for c, v in events.items()}


def summarize(values):
    if not values:
        return "n/a"
    return f"n={len(values):<4} median={median(values):6.2f}s  max={max(values):6.2f}s"


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 1

    path, slow = sys.argv[1], sys.argv[2]
    events = parse(path)

    if slow not in events:
        print(f"slow chain {slow} produced no batches — nothing to attribute")
        return 1

    slow_marks = events[slow]
    fast_ids = sys.argv[3:] or [c for c in events if c != slow]

    span = max(max(v) for v in events.values()) - min(min(v) for v in events.values())
    print(f"window {span.total_seconds():.0f}s   slow chain {slow}: {len(slow_marks)} batches\n")
    print(f"{'chain':<8} {'gaps overlapping slow-chain work':<42} {'gaps with the slow chain idle'}")
    print("-" * 110)

    for chain in sorted(fast_ids, key=int):
        marks = events[chain]
        if len(marks) < 3:
            print(f"{chain:<8} too few batches to compare")
            continue

        overlapped, clear = [], []
        for prev, cur in zip(marks, marks[1:]):
            gap = (cur - prev).total_seconds()
            # Did the slow chain start a batch inside this gap?
            if any(prev < s < cur for s in slow_marks):
                overlapped.append(gap)
            else:
                clear.append(gap)

        print(f"{chain:<8} {summarize(overlapped):<42} {summarize(clear)}")

    print(
        "\nCoupled   => overlapping gaps are markedly longer: the sibling was "
        "waiting on the slow chain's batch.\nDecoupled => the two columns "
        "match: the slow chain's work is invisible to its siblings."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
