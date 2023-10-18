#!/bin/bash
psql -U postgres graph_node -c "GRANT ALL PRIVILEGES ON TABLE public.ens_names TO graph;"

expected_hash="a6316b1e7770b1f3142f1f21d4248b849a5c6eb998e3e66336912c9750c41f31"
file="./ens_names.sql.gz"
url="https://storage.cloud.google.com/ens-files/ens_names.sql.gz"

if [ ! -f "$file" ]; then
    curl $url --output $file
else
    echo "File $file already exists. Not downloading."
fi

# Calculate the actual hash
actual_hash=$(sha256sum "$file" | awk '{print $1}')
# Check if the actual hash matches the expected hash
if [[ "$actual_hash" != "$expected_hash" ]]; then
  echo "Hash does not match! Actual hash: $actual_hash"
  exit 1  # Exit with non-zero code
fi

zcat ens_names.sql.gz | psql -U postgres graph_node

