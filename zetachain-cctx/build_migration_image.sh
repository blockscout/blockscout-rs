#!/bin/bash

echo "Building migration Docker image..."

# Build the migration image
docker build -f Dockerfile.migration -t zetachain-cctx-migration:latest .

echo "Migration image built successfully!"
echo ""
echo "Usage examples:"
echo ""
echo "1. Run migrations:"
echo "   docker run --rm --network host \\"
echo "     -e DATABASE_URL='postgresql://postgres:postgres@localhost:5433/zetachain_cctx' \\"
echo "     zetachain-cctx-migration:latest"
echo ""
echo "2. Run migrations with custom database URL:"
echo "   docker run --rm --network host \\"
echo "     -e DATABASE_URL='postgresql://user:pass@host:port/db' \\"
echo "     zetachain-cctx-migration:latest"
echo ""
echo "3. Connect to database container and run migrations:"
echo "   docker run --rm --network container:zetachain-cctx-database \\"
echo "     -e DATABASE_URL='postgresql://postgres:postgres@localhost:5432/zetachain_cctx' \\"
echo "     zetachain-cctx-migration:latest"
echo ""
echo "4. Interactive mode (for debugging):"
echo "   docker run --rm -it --network host \\"
echo "     -e DATABASE_URL='postgresql://postgres:postgres@localhost:5433/zetachain_cctx' \\"
echo "     zetachain-cctx-migration:latest bash" 