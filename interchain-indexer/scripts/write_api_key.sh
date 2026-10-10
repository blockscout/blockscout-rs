#!/usr/bin/env bash
# Prints the env variable that configures a write API key, so nobody hashes a
# key by hand. The service stores only the SHA-256 digest of a key and hashes
# the presented `x-api-key` the same way (interchain-indexer-server/src/auth.rs).
#
#   write_api_key.sh new <name>     generate a key, print it and its variable
#   write_api_key.sh digest <name>  read an existing key from stdin (hidden on
#                                   a terminal), print its variable only
#
# Procedure: .memory-bank/runbooks/write-api.md, section "Keys".
set -euo pipefail

usage() {
    echo "usage: $0 new <name> | digest <name>" >&2
    exit 2
}

[[ $# -eq 2 ]] || usage
mode=$1
name=$2

# The name is the suffix of the env variable and, lowercased by the settings
# loader, the actor in the audit log. `__` would nest it one level deeper, and a
# leading or trailing `_` would merge into the `__` separator.
if [[ ! $name =~ ^[A-Za-z0-9]+(_[A-Za-z0-9]+)*$ ]]; then
    echo "invalid key name '$name': use letters, digits and single underscores, e.g. ops_alice" >&2
    exit 2
fi
env_name=$(tr '[:lower:]' '[:upper:]' <<<"$name")
actor=$(tr '[:upper:]' '[:lower:]' <<<"$name")

case $mode in
    new)
        key=$(openssl rand -hex 32)
        ;;
    digest)
        key=
        if [[ -t 0 ]]; then
            read -rsp "key for $actor (input hidden): " key || true
            echo >&2
        else
            IFS= read -r key || true
        fi
        if [[ -z $key ]]; then
            echo "empty key" >&2
            exit 2
        fi
        ;;
    *)
        usage
        ;;
esac

# `printf %s`, not `echo`: a trailing newline would change the digest.
digest=$(printf %s "$key" | openssl dgst -sha256 -r | cut -d' ' -f1)
variable="INTERCHAIN_INDEXER__WRITE_API__KEYS_SHA256__${env_name}=${digest}"

if [[ $mode == new ]]; then
    cat <<EOF
Write API key "$actor" (the actor in write_api_audit_log).

1. Give this key to its owner over a secret channel. It is shown only once and
   stored nowhere; if it is lost, generate a new one.

   $key

2. Add this variable to the service's deployment env (it holds the digest, not
   the key) and restart the service: keys are read at startup.

   $variable
EOF
else
    cat <<EOF
Add this variable to the service's deployment env and restart the service:

$variable
EOF
fi
