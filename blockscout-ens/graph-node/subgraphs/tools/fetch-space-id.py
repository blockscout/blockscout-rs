import requests
import json
from web3 import Web3
import os

url = os.environ.get("RPC_URL")
if not url:
    raise Exception("RPC_URL environment variable is not set")
contract_address = os.environ.get("CONTRACT")
if not contract_address:
    raise Exception("CONTRACT environment variable is not set")

base_node = os.environ.get("BASE_NODE", "eth").strip('.').lower()


def get_identifier(url, contract_address):
    data = {
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [
            {
                "to": contract_address,
                # identifier() function signature
                "data": "0x7998a1c4"
            },
            "latest"
        ],
        "id": 1
    }

    response = requests.post(url, data=json.dumps(data))
    if response.ok:
        return response.json()["result"]
    else:
        raise Exception(f"Failed to fetch identifier: {response.text}")


def get_empty_label_hash(identifier):
    return Web3.keccak(bytes([0] * 32) + identifier.to_bytes(32, "big"))

def get_base_node_hash(tld: str, empty_label_hash):
    return Web3.keccak(empty_label_hash + Web3.keccak(tld.encode()))

identifier = get_identifier(url, contract_address)
empty_label_hash = get_empty_label_hash(int(identifier, 16))
base_node_hash = get_base_node_hash(base_node, empty_label_hash)

print(f"""
identifier:       '{identifier}'
empty_label_hash: '{empty_label_hash.hex()}'
base_node:        '{base_node}'
base_node_hash:   '{base_node_hash.hex()}'
""")
