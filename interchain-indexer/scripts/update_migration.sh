#!/bin/bash

# Script to generate migration and update the generated file with custom content

# Check if name argument is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <migration_name>"
    exit 1
fi

MIGRATION_NAME="$1"
MIGRATION_DIR="interchain-indexer-migration"

# Run the sea-orm-cli command and capture output
echo "Generating new migration..."
OUTPUT=$(sea-orm-cli migrate generate -d "$MIGRATION_DIR" "$MIGRATION_NAME" 2>&1)

# Check if command was successful
if [ $? -ne 0 ]; then
    echo "Error generating migration:"
    echo "$OUTPUT"
    exit 1
fi

echo "$OUTPUT"

# Extract the file path from the output
# The output format is: "Creating migration file `interchain-indexer-migration/src/m20250801_123141_hello.rs`"
FILE_PATH=$(echo "$OUTPUT" | grep "Creating migration file" | sed "s/Creating migration file \`\(.*\)\`/\1/")

if [ -z "$FILE_PATH" ]; then
    echo "Error: Could not extract file path from output"
    exit 1
fi

echo "Extracted file path: $FILE_PATH"

# Extract the migration name from the file path
# From: interchain-indexer-migration/src/m20250801_123141_hello.rs
# To: m20250801_123141_hello
MIGRATION_FILE_NAME=$(basename "$FILE_PATH" .rs)

echo "Migration file name: $MIGRATION_FILE_NAME"

# Create the new content
NEW_CONTENT="use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, include_str!(\"migrations_up/${MIGRATION_FILE_NAME}_up.sql\")).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, include_str!(\"migrations_down/${MIGRATION_FILE_NAME}_down.sql\")).await?;
        Ok(())
    }
}"

# Update the file
echo "$NEW_CONTENT" > "$FILE_PATH"

echo "Updated migration file: $FILE_PATH"

mkdir -p "$MIGRATION_DIR/src/migrations_up"
mkdir -p "$MIGRATION_DIR/src/migrations_down"
touch "$MIGRATION_DIR/src/migrations_up/${MIGRATION_FILE_NAME}_up.sql"
touch "$MIGRATION_DIR/src/migrations_down/${MIGRATION_FILE_NAME}_down.sql" 

echo "Created migration files:"
echo "  - $MIGRATION_DIR/src/migrations_up/${MIGRATION_FILE_NAME}_up.sql"
echo "  - $MIGRATION_DIR/src/migrations_down/${MIGRATION_FILE_NAME}_down.sql" 