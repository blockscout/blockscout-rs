#!/usr/bin/env bash

set -euo pipefail

MULTICHAIN_AGGREGATOR_URL="${MULTICHAIN_AGGREGATOR_URL:-http://localhost:8050}"
MULTICHAIN_AGGREGATOR_API_KEY="${MULTICHAIN_AGGREGATOR_API_KEY:-b7e2e2e2-2b6e-4e2a-9c7a-1e2b2e2e2e2e}"
CHAIN_ID="${CHAIN_ID:-10}"

N="${1:-10}"  # Default number of entities

# --- Utilities ---

# Generate random hex string of N bytes, prefixed with 0x
random_hex_bytes() {
  local nbytes=$1
  openssl rand -hex "$nbytes" | tr '[:upper:]' '[:lower:]'
}

# 20-byte hex for addresses
random_address_hash() {
  echo "$(random_hex_bytes 20)"
}

# 32-byte hex for hashes
random_32byte_hash() {
  echo "$(random_hex_bytes 32)"
}

random_number() {
  shuf -i 1-"$1" -n 1
}

random_token_type() {
  types=("ERC-20" "ERC-721" "ERC-1155" "ERC-404" "ERC7802")
  echo "${types[$(shuf -i 0-4 -n 1)]}"
}

random_block_type() {
  types=("BLOCK" "TRANSACTION")
  echo "${types[$(shuf -i 0-1 -n 1)]}"
}

timestamp_now() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# --- Send JSON payload to API and print result ---

import_batch() {
  local payload="$1"
  local entity_name="$2"

  http_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$MULTICHAIN_AGGREGATOR_URL/api/v1/import:batch" \
    -H "Content-Type: application/json" \
    -d "$payload")

  if [[ "$http_code" -ge 200 && "$http_code" -lt 300 ]]; then
    echo "✅ OK"
  else
    echo "❌ Error (HTTP $http_code)"
    echo "Input payload: $payload"
  fi
}


# --- Entity Generators ---

send_addresses() {
  echo -n "Sending $N addresses... "
  payload=$(jq -n \
    --arg chain_id "$CHAIN_ID" \
    --arg api_key "$MULTICHAIN_AGGREGATOR_API_KEY" \
    --argjson addresses "$(for i in $(seq 1 "$N"); do
      jq -n \
        --arg hash "0x$(random_address_hash)" \
        --arg ens_name "name-$i.eth" \
        --arg contract_name "Contract $i" \
        --arg token_name "Token $i" \
        --arg token_type "$(random_token_type)" \
        --argjson is_contract true \
        --argjson is_verified_contract false \
        --argjson is_token true \
        '{hash: $hash, ens_name: $ens_name, contract_name: $contract_name, token_name: $token_name, token_type: $token_type, is_contract: $is_contract, is_verified_contract: $is_verified_contract, is_token: $is_token}'
    done | jq -s .)" \
    '{chain_id: $chain_id, addresses: $addresses, api_key: $api_key}'
  )

  import_batch "$payload" "addresses"
}

send_block_ranges() {
  echo -n "Sending 1 block_range... "
  
  local min=$(random_number 1000)
  local max=$((min + $(random_number 1000000)))

  payload=$(jq -n \
    --arg chain_id "$CHAIN_ID" \
    --arg api_key "$MULTICHAIN_AGGREGATOR_API_KEY" \
    --arg min_block_number "$min" \
    --arg max_block_number "$max" \
    '{
      chain_id: $chain_id,
      block_ranges: [
        {
          min_block_number: $min_block_number,
          max_block_number: $max_block_number
        }
      ],
      api_key: $api_key
    }'
  )

  import_batch "$payload" "block_ranges"
}

send_hashes() {
  echo -n "Sending $N hashes... "
  payload=$(jq -n \
    --arg chain_id "$CHAIN_ID" \
    --arg api_key "$MULTICHAIN_AGGREGATOR_API_KEY" \
    --argjson hashes "$(for i in $(seq 1 "$N"); do
      jq -n \
        --arg hash "0x$(random_32byte_hash)" \
        --arg hash_type "$(random_block_type)" \
        '{hash: $hash, hash_type: $hash_type}'
    done | jq -s .)" \
    '{chain_id: $chain_id, hashes: $hashes, api_key: $api_key}'
  )

  import_batch "$payload" "hashes"
}

send_address_coin_balances() {
  echo -n "Sending $N address_coin_balances... "
  payload=$(jq -n \
    --arg chain_id "$CHAIN_ID" \
    --arg api_key "$MULTICHAIN_AGGREGATOR_API_KEY" \
    --argjson address_coin_balances "$(for i in $(seq 1 "$N"); do
      jq -n \
        --arg address_hash "0x$(random_address_hash)" \
        --arg value "$(random_number 1000000000)" \
        '{address_hash: $address_hash, value: $value}'
    done | jq -s .)" \
    '{chain_id: $chain_id, address_coin_balances: $address_coin_balances, api_key: $api_key}'
  )

  import_batch "$payload" "address_coin_balances"
}

send_tokens() {
  echo -n "Sending $N tokens... "
  payload=$(jq -n \
    --arg chain_id "$CHAIN_ID" \
    --arg api_key "$MULTICHAIN_AGGREGATOR_API_KEY" \
    --argjson tokens "$(for i in $(seq 1 "$N"); do
      jq -n \
        --arg address_hash "0x$(random_address_hash)" \
        --arg name "Token $i" \
        --arg symbol "TK$i" \
        --argjson decimals 18 \
        --arg token_type "$(random_token_type)" \
        --arg icon_url "https://example.com/icon$i.png" \
        --arg total_supply "$(random_number 1000000000)" \
        --arg fiat_value "$(random_number 1000).$(random_number 99)" \
        --arg circulating_market_cap "$(random_number 1000000000)" \
        --arg holders_count "$(random_number 10000)" \
        --arg transfers_count "$(random_number 100000)" \
        '{
          address_hash: $address_hash,
          metadata: {
            name: $name,
            symbol: $symbol,
            decimals: $decimals,
            token_type: $token_type,
            icon_url: $icon_url,
            total_supply: $total_supply
          },
          price_data: {
            fiat_value: $fiat_value,
            circulating_market_cap: $circulating_market_cap
          },
          counters: {
            holders_count: $holders_count,
            transfers_count: $transfers_count
          }
        }'
    done | jq -s .)" \
    '{chain_id: $chain_id, tokens: $tokens, api_key: $api_key}'
  )

  import_batch "$payload" "tokens"
}

send_counters() {
  echo "Sending $N counter entries... "
  
  for i in $(seq 1 "$N"); do
    echo -n "  Day $i: "
    
    local daily_transactions=$(random_number 1000000)
    local total_transactions=$((daily_transactions + $(random_number 10000000)))
    local total_addresses=$(random_number 100000)
    local days_ago=$(random_number "$N")
    local timestamp=$(($(date -u +%s) - (days_ago * 86400)))
    
    payload=$(jq -n \
      --arg chain_id "$CHAIN_ID" \
      --arg api_key "$MULTICHAIN_AGGREGATOR_API_KEY" \
      --arg timestamp "$timestamp" \
      --arg daily_transactions_number "$daily_transactions" \
      --arg total_transactions_number "$total_transactions" \
      --arg total_addresses_number "$total_addresses" \
      '{
        chain_id: $chain_id,
        counters: {
          timestamp: $timestamp,
          global_counters: {
            daily_transactions_number: $daily_transactions_number,
            total_transactions_number: $total_transactions_number,
            total_addresses_number: $total_addresses_number
          }
        },
        api_key: $api_key
      }'
    )

    import_batch "$payload" "counters"
  done
}

# --- Execute all imports ---

# send_addresses
# send_block_ranges
# send_hashes
# send_address_coin_balances
# send_tokens
send_counters

