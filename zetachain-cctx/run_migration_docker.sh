#!/bin/bash

# Migration runner script
# This script provides various ways to run migrations using just and sea-orm-cli

set -e

# Default values
DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:postgres@localhost:5433/zetachain_cctx"}
IMAGE_NAME="zetachain-cctx-migration:latest"

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS] [COMMAND]"
    echo ""
    echo "Options:"
    echo "  -d, --database-url URL    Database URL (default: $DATABASE_URL)"
    echo "  -i, --image IMAGE         Migration image name (default: $IMAGE_NAME)"
    echo "  -h, --help                Show this help message"
    echo ""
    echo "Commands:"
    echo "  build                     Build the migration image"
    echo "  migrate-up                Run sea-orm migrations up"
    echo "  migrate-down              Run sea-orm migrations down"
    echo "  new-migration NAME        Generate new migration"
    echo "  populate                  Populate flattened fields after migration"
    echo "  interactive               Run in interactive mode"
    echo "  list                      List available just commands"
    echo ""
    echo "Examples:"
    echo "  $0 build"
    echo "  $0 migrate-up"
    echo "  $0 -d 'postgresql://user:pass@host:port/db' migrate-up"
    echo "  $0 new-migration add_new_field"
    echo "  $0 populate"
}

# Function to build the image
build_image() {
    echo "Building migration image..."
    docker build -f Dockerfile.migration -t "$IMAGE_NAME" .
    echo "Migration image built successfully!"
}

# Function to run just command
run_just() {
    local command="$1"
    echo "Running: just $command"
    docker run --rm --network host \
        -e DATABASE_URL="$DATABASE_URL" \
        -v "$(pwd):/app" \
        -w /app \
        "$IMAGE_NAME" just "$command"
}

# Function to run migrations up
run_migrate_up() {
    echo "Running sea-orm migrations up..."
    run_just "migrate-up"
}

# Function to run migrations down
run_migrate_down() {
    echo "Running sea-orm migrations down..."
    run_just "migrate-down"
}

# Function to generate new migration
generate_migration() {
    local name="$1"
    if [ -z "$name" ]; then
        echo "Error: Migration name is required"
        echo "Usage: $0 new-migration MIGRATION_NAME"
        exit 1
    fi
    echo "Generating new migration: $name"
    run_just "new-migration $name"
}

# Function to check available commands
list_commands() {
    echo "Available just commands:"
    run_just "--list"
}

# Function to run in interactive mode
run_interactive() {
    echo "Starting interactive migration session..."
    docker run --rm -it --network host \
        -e DATABASE_URL="$DATABASE_URL" \
        -v "$(pwd):/app" \
        -w /app \
        "$IMAGE_NAME" bash
}

# Function to populate flattened fields
populate_fields() {
    echo "Populating flattened fields..."
    docker exec -i zetachain-cctx-database psql -U postgres -d zetachain_cctx < populate_flattened_fields_batch.sql
    echo "Data population completed!"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--database-url)
            DATABASE_URL="$2"
            shift 2
            ;;
        -i|--image)
            IMAGE_NAME="$2"
            shift 2
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        build)
            build_image
            exit 0
            ;;
        migrate-up)
            run_migrate_up
            exit 0
            ;;
        migrate-down)
            run_migrate_down
            exit 0
            ;;
        new-migration)
            generate_migration "$2"
            exit 0
            ;;
        populate)
            populate_fields
            exit 0
            ;;
        interactive)
            run_interactive
            exit 0
            ;;
        list)
            list_commands
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# If no command specified, show usage
show_usage 