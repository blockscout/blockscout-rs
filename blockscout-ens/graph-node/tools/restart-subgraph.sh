#!/bin/bash
set -euo pipefail
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# Load environment for graphman
if [ -f /app/.env ]; then
  set -a
  . /app/.env
  set +a
fi

CONFIG="/app/config/graph-node.toml"
THRESHOLD=1000
AUTO_YES=0

while [ $# -gt 0 ]; do
  case "$1" in
    -y|--yes)
      AUTO_YES=1
      shift
      ;;
    -t|--threshold)
      THRESHOLD="$2"
      shift 2
      ;;
    *)
      echo "Usage: $0 [-y|--yes] [-t|--threshold N]"
      exit 1
      ;;
  esac
done

tmpfile="$(mktemp)"
trap 'rm -f "$tmpfile"' EXIT

graphman -c "$CONFIG" info -s --all | awk -F'|' -v threshold="$THRESHOLD" '
function trim(x) {
  gsub(/^[ \t]+|[ \t]+$/, "", x)
  return x
}
function flush() {
  if (namespace != "" && synced && behind > threshold) {
    printf "%s|%s|%s|%s\n", namespace, version, chain, behind
  }
}
$1 ~ /Namespace/ {
  flush()
  namespace = trim($2)
  sub(/ \[primary\]$/, "", namespace)
  version = ""
  chain = ""
  behind = 0
  synced = 0
}
$1 ~ /Version/ {
  version = trim($2)
  sub(/ \(current\)$/, "", version)
}
$1 ~ /Chain$/ { chain = trim($2) }
$1 ~ /Synced/ { synced = ($2 ~ /true/) }
$1 ~ /Blocks behind/ { behind = $2 + 0 }
END { flush() }
' > "$tmpfile"

if [ ! -s "$tmpfile" ]; then
  echo "No stuck subgraphs found (threshold: $THRESHOLD blocks)."
  exit 0
fi

echo "Stuck subgraphs (threshold: $THRESHOLD blocks):"
echo
awk -F'|' '{ printf "  - %-8s | %-35s | %-20s | behind=%s\n", $1, $2, $3, $4 }' "$tmpfile"
echo
count="$(wc -l < "$tmpfile" | tr -d " ")"
echo "Total: $count"

if [ "$AUTO_YES" -ne 1 ]; then
  printf "Restart these subgraphs? [y/N]: "
  read -r answer
  case "$answer" in
    y|Y) ;;
    *) echo "Aborted."; exit 0 ;;
  esac
fi

echo
echo "Restarting..."

while IFS='|' read -r namespace version chain behind; do
  echo "Restarting $namespace ($version, chain=$chain, behind=$behind)"
  graphman -c "$CONFIG" restart "$namespace"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    echo "OK: $namespace"
  else
    echo "FAILED: $namespace (exit=$rc)"
  fi
  echo
done < "$tmpfile"