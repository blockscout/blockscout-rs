#!/usr/bin/env bash
# Prints the env variable that configures a write API key, so nobody hashes a
# key by hand. The service stores only the SHA-256 digest of a key and hashes
# the presented `x-api-key` the same way (interchain-indexer-server/src/auth.rs).
#
#   write_api_key.sh new <name>     generate a key, show it and print its variable
#   write_api_key.sh digest <name>  read an existing key from stdin (hidden on
#                                   a terminal), print its variable only
#
# Procedure: .memory-bank/runbooks/write-api.md, section "Keys".
set -euo pipefail

# A generated key is `iiwk_` ("interchain-indexer write key") plus 32 base62
# characters (about 190 bits). The prefix and the mixed case keep it visually
# distinct from its 64-hex digest, and let secret scanners recognise it.
KEY_PREFIX=iiwk_
KEY_BODY_LENGTH=32

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

generate_key() {
    local body=
    # Dropping `+` and `/` from uniform base64 leaves uniform base62.
    while ((${#body} < KEY_BODY_LENGTH)); do
        body+=$(openssl rand -base64 48 | tr -dc 'A-Za-z0-9')
    done
    printf '%s%s' "$KEY_PREFIX" "${body:0:KEY_BODY_LENGTH}"
}

case $mode in
    new)
        key=$(generate_key)
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

print_variable() {
    cat <<EOF
Add this variable to the service's deployment env (it holds the digest, not the
key) and restart the service: keys are read at startup.

$variable
EOF
}

if [[ $mode == digest ]]; then
    print_variable
    exit 0
fi

if [[ -t 0 && -t 1 ]]; then
    # Show the key on the terminal's alternate screen, like `less` does: when it
    # closes, the key is gone from the screen and is not kept in the scrollback.
    printf '\033[?1049h\033[H'
    # Leave the alternate screen on every exit, Ctrl-C included.
    trap 'printf "\033[?1049l"' EXIT
    cat <<EOF
Write API key "$actor" (the actor in write_api_audit_log):

    $key

Copy it straight into its owner's secret store. Neither this tool nor the
service keeps it; if it is lost, generate a new one.

Press Enter to close this screen.
EOF
    read -rs _ || true
    printf '\033[?1049l'
    trap - EXIT
    echo "The key for \"$actor\" was shown on a temporary screen; it is not in this terminal's scrollback."
    echo
    print_variable
else
    # Not a terminal: the caller captures the output, and the key stays wherever
    # it is written.
    cat <<EOF
Write API key "$actor" (the actor in write_api_audit_log). Neither this tool
nor the service keeps it; it stays wherever this output was written.

    $key

EOF
    print_variable
fi
