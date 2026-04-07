cat > /root/initial-cron-restart-subgraph.sh <<'BOOTSTRAP'
#!/bin/bash
set -euo pipefail

PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

echo "[1/7] Updating apt and installing cron..."
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y cron

echo "[2/7] Ensuring directories/files exist..."
mkdir -p /app
touch /app/.env
chmod 600 /app/.env

echo "[3/7] Exporting postgres_* vars to /app/.env ..."
tmp_env="$(mktemp)"
trap 'rm -f "$tmp_env"' EXIT

# Prefer current shell environment first.
env | awk -F= '
  $1 ~ /^postgres_/ {
    key=$1
    sub(/^[^=]*=/, "", $0)
    val=$0
    gsub(/\\/,"\\\\",val)
    gsub(/"/,"\\\"",val)
    printf("export %s=\"%s\"\n", key, val)
  }
' > "$tmp_env"

# Fallback: if current shell has none, try PID 1 environment.
if [ ! -s "$tmp_env" ] && [ -r /proc/1/environ ]; then
  tr '\0' '\n' </proc/1/environ | awk -F= '
    $1 ~ /^postgres_/ {
      key=$1
      sub(/^[^=]*=/, "", $0)
      val=$0
      gsub(/\\/,"\\\\",val)
      gsub(/"/,"\\\"",val)
      printf("export %s=\"%s\"\n", key, val)
    }
  ' > "$tmp_env"
fi

if [ ! -s "$tmp_env" ]; then
  echo "ERROR: no postgres_* variables found in current shell environment or /proc/1/environ"
  echo "Check with: env | grep '^postgres_'"
  exit 1
fi

cp "$tmp_env" /app/.env
chmod 600 /app/.env

echo "Saved these variables to /app/.env:"
sed 's/^export /  /' /app/.env

echo "[4/7] Creating /root/restart-subgraph.sh ..."
cat >/root/restart-subgraph.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

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

echo "==== $(date) restart-subgraph started ===="
echo "Using CONFIG=$CONFIG THRESHOLD=$THRESHOLD"
echo

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
  if graphman -c "$CONFIG" restart "$namespace"; then
    echo "OK: $namespace"
  else
    echo "FAILED: $namespace"
  fi
  echo
done < "$tmpfile"

echo "==== $(date) restart-subgraph finished ===="
SCRIPT

chmod +x /root/restart-subgraph.sh

echo "[5/7] Installing root crontab ..."
cat >/tmp/root-crontab <<'CRON'
SHELL=/bin/bash
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

0 0 * * * /bin/bash /root/restart-subgraph.sh -y -t 5000 >> /var/log/restart_subgraph.log 2>&1
CRON

crontab /tmp/root-crontab
rm -f /tmp/root-crontab

echo "[6/7] Starting/restarting cron ..."
touch /var/log/restart_subgraph.log
chmod 644 /var/log/restart_subgraph.log

if command -v service >/dev/null 2>&1; then
  service cron restart || service cron start
else
  /usr/sbin/cron
fi

echo "[7/7] Done."
echo
echo "Installed crontab:"
crontab -l
echo
echo "Current /app/.env:"
cat /app/.env
echo
echo "Manual test:"
echo "  /bin/bash /root/restart-subgraph.sh -y -t 5000"
echo
echo "Cron log:"
echo "  tail -f /var/log/restart_subgraph.log"
BOOTSTRAP

chmod +x /root/initial-cron-restart-subgraph.sh
/root/initial-cron-restart-subgraph.sh
