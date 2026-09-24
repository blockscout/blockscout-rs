#!/bin/bash

echo "Building migration Docker image with just and sea-orm-cli..."

# Build the migration image
docker build -f Dockerfile.migration -t zetachain-cctx-migration:latest .

echo "Migration image built successfully!"
echo ""
echo "Usage examples:"
echo ""
echo "1. Run sea-orm migrations up:"
echo "   ./run_migration_docker.sh migrate-up"
echo ""
echo "2. Run sea-orm migrations down:"
echo "   ./run_migration_docker.sh migrate-down"
echo ""
echo "3. Generate new migration:"
echo "   ./run_migration_docker.sh new-migration add_new_field"
echo ""
echo "4. List available just commands:"
echo "   ./run_migration_docker.sh list"
echo ""
echo "5. Interactive mode:"
echo "   ./run_migration_docker.sh interactive"
echo ""
echo "6. Populate flattened fields after migration:"
echo "   ./run_migration_docker.sh populate"
echo ""
echo "7. With custom database URL:"
echo "   ./run_migration_docker.sh -d 'postgresql://user:pass@host:port/db' migrate-up" 