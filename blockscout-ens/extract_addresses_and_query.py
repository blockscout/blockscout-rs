#!/usr/bin/env python3
"""
Script to extract Ethereum addresses from a log file and execute SQL queries with them.
"""

import re
import sys
import os
import logging
from sqlalchemy import create_engine, text
from typing import List, Optional
import time

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

def extract_addresses_from_file(file_path: str) -> List[str]:
    """
    Extract Ethereum addresses from a log file.
    
    Args:
        file_path: Path to the log file
        
    Returns:
        List of Ethereum addresses found in the file
    """
    addresses = set()
    
    try:
        with open(file_path, 'r') as file:
            content = file.read()
            # Extract addresses using regex pattern
            pattern = r'"address":"(0x[a-fA-F0-9]{40})"'
            matches = re.findall(pattern, content)
            
            # Add all unique addresses to the set
            for address in matches:
                addresses.add(address)
                
        logger.info(f"Extracted {len(addresses)} unique Ethereum addresses from {file_path}")
        return list(addresses)
    
    except FileNotFoundError:
        logger.error(f"File not found: {file_path}")
        return []
    except Exception as e:
        logger.error(f"Error extracting addresses from file: {e}")
        return []

def connect_to_database(database_url: str) -> Optional[object]:
    """
    Connect to the database.
    
    Args:
        database_url: Database connection string
        
    Returns:
        SQLAlchemy engine object or None if connection fails
    """
    try:
        engine = create_engine(database_url)
        # Test connection
        with engine.connect() as connection:
            logger.info("Successfully connected to the database")
        return engine
    except Exception as e:
        logger.error(f"Error connecting to database: {e}")
        return None

def execute_queries(engine: object, addresses: List[str], sql_query: str) -> None:
    """
    Execute SQL queries for each address.
    
    Args:
        engine: SQLAlchemy engine object
        addresses: List of Ethereum addresses
        sql_query: SQL query template with parameter $1
    """
    # Replace $1 with SQLAlchemy parameter format
    query = sql_query.replace("$1", ":address")
    
    successful_queries = 0
    failed_queries = 0
    total_elapsed_time = 0
    with engine.connect() as connection:
        for address in addresses:
            try:
                start_time = time.time()
                result = connection.execute(text(query), {"address": address.lower()})
                rows = result.fetchall()
                elapsed_time = time.time() - start_time
                logger.info(f"Query for address {address} returned {len(rows)} rows in {elapsed_time:.2f} seconds")
                successful_queries += 1
            except Exception as e:
                logger.error(f"Error executing query for address {address}: {e}")
                failed_queries += 1
            finally:
                total_elapsed_time += time.time() - start_time
    
    average_time = total_elapsed_time / len(addresses)
    logger.info(f"Executed {successful_queries} successful queries in {total_elapsed_time:.2f} seconds, average time per query: {average_time:.2f} seconds")
    if failed_queries > 0:
        logger.warning(f"Failed to execute {failed_queries} queries")

def main():
    """
    Main function to execute the script.
    """
    
    
    log_file_path = sys.argv[1] if len(sys.argv) > 1 else '/Users/levlymarenko/Downloads/logslogs.logs'
    database_url = 'postgresql://graph:8DNBw4Yk28yP60InaY8IzlpiNIcERVfvrcA6ObjoZcbOUtAqlJNa7ASfSbwuicoT@localhost:15432/graph_node?sslmode=disable'
    sql_query = '''SELECT * FROM (SELECT
vid,
id,
name,
resolved_address,
resolver,
created_at,
to_timestamp(created_at) as registration_date,
owner,
wrapped_owner,
stored_offchain,
resolved_with_wildcard,
to_timestamp(expiry_date) as expiry_date,
COALESCE(to_timestamp(expiry_date) < now(), false) AS is_expired
, 'ens' AS "protocol_slug" FROM "sgd1"."domain" WHERE (block_range @> 2147483647) AND (label_name IS NOT NULL) AND (name NOT LIKE '%[%') AND (
(
    expiry_date is null
    OR to_timestamp(expiry_date) > now()
)
) AND (($1 <> $1) OR (resolved_address = $1) OR (owner = $1) OR (wrapped_owner = $1)) ORDER BY "created_at" ASC LIMIT 51) AS "sub" ORDER BY "created_at" ASC LIMIT 51
'''
    
    # Extract addresses from the log file
    addresses = extract_addresses_from_file(log_file_path)
    if not addresses:
        logger.error("No addresses found in the log file")
        sys.exit(1)
    
    # Connect to the database
    engine = connect_to_database(database_url)
    if not engine:
        logger.error("Failed to connect to the database")
        sys.exit(1)
    
    # Execute queries
    execute_queries(engine, addresses, sql_query)
    
    logger.info("Script execution completed")

if __name__ == "__main__":
    main()

