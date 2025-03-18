#!/bin/bash

# Define the configuration file path
CONFIG_FILE="/root/config.toml"

# Create the configuration file only if it does not exist
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "[general]\n\n\
[store]\n\
[store.primary]\n\
connection = \"postgresql://$postgres_user:$postgres_pass@$postgres_host:5432/$postgres_db\"\n\
weight = 1\n\
pool_size = 10\n\n\
[chains]\n\
ingestor = \"block_ingestor_node\"\n\n\
[deployment]\n\
[[deployment.rule]]\n\
shard = \"primary\"\n\
indexers = [ \"default\" ]" > "$CONFIG_FILE"
fi

# Use graphman to retrieve and process data
STUCK_DEPLOYMENTS=$(graphman -c "$CONFIG_FILE" info --all -s | \
awk '
BEGIN { FS="|"; OFS="|" }
/^name[ ]+\|/ { name=$2 }
/^latest block[ ]+\|/ { latest=$2 }
/^chain head block[ ]+\|/ {
    chain_head=$2
    if (chain_head - latest > 100) {
        print name
    }
}
' | tr -d " ")

# Debug: Print number of stuck deployments and their names
echo "Number of stuck deployments: $(echo "$STUCK_DEPLOYMENTS" | wc -w)"
echo "Names of stuck deployments: $STUCK_DEPLOYMENTS"

# Restart each stuck deployment with a debug message
echo "$STUCK_DEPLOYMENTS" | xargs -I{} sh -c 'echo "Restarting subgraph: {}"; graphman -c "$CONFIG_FILE" restart {}'
