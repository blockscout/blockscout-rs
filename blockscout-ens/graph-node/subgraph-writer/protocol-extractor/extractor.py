import sys
import os
import requests
import itertools
import argparse
import json
from events import events_from_abi, abi_from_str, similar_event_exists, only_events
import yaml

TODO = "TODO: fill"

def parse_arguments():
    parser = argparse.ArgumentParser(description="Extract ENS contracts")
    parser.add_argument(
        '-i', '--input-abis',
        default='output-abis.json',
        help='Specify the input file with abis of contracts'
    )

    parser.add_argument(
        '-e', '--extra',
        nargs='*',
        type=str,
        default=[],
        help='Specify additional abis'
    )

    parser.add_argument(
        '-c', '--config',
        default='contract_events.json',
        help='Specify the config file'
    )

    # Add argument for 'output'
    parser.add_argument(
        '-o', '--output',
        default='output-protocol.yaml',
        type=str,
        help='Specify the output file'
    )

    # Parse the command line arguments
    args = parser.parse_args()

    return args


def get_hash_from_dict(data):
    data_str = str(data)
    hash_value = hash(data_str)
    hex_hash = format(hash_value, 'x')
    return hex_hash

def get_config(contract_name, result):
    if result.get(contract_name):
        config = {
            contract_name: True,
            f"{contract_name}_name": result[contract_name]['default_name'],
            f"{contract_name}_address": result[contract_name]['address'],
            f"{contract_name}_start_block": TODO,
            f"{contract_name}_address_test": TODO,
            f"{contract_name}_start_block_test": TODO,
            f"{contract_name}_abi": json.dumps(result[contract_name]['abi']),
        }
        if contract_name == 'resolver':
            config.pop(f"{contract_name}_address")
            config.pop(f"{contract_name}_start_block_test")
        return config
    else:
        return {
            contract_name: False,
            f"{contract_name}_name": None,
            f"{contract_name}_address": None,
            f"{contract_name}_start_block": None,
            f"{contract_name}_address_test": None,
            f"{contract_name}_start_block_test": None,
            f"{contract_name}_abi": None,
        }
    


def main():
    args = parse_arguments()
    if os.path.exists(args.input_abis):
        with open(args.input_abis, 'r') as f:
            abis: dict = json.load(f)
    else:
        abis: dict = {}

    for extra_abi in args.extra:
        extra_abi = abi_from_str(extra_abi)
        hash = get_hash_from_dict(extra_abi)
        abis[hash] = extra_abi

    with open(args.config, 'r') as f:
        config: dict = json.load(f)

    result = {}
    used_addresses = set()
    for (contract_name, contract) in config.items():
        for (address, abi) in abis.items():
            if address in used_addresses:
                continue
            events = only_events(abi)
            similar_events = [
                similar_event_exists(event, events)
                for event in contract['events']
            ]
            if all(similar_events):
                if not contract_name in result:
                    result[contract_name] = {"abi": abi, "address": address, "default_name": contract['default_name']}
                    used_addresses.add(address)
                    break
    configs = [
        get_config(contract_name, result)
        for contract_name in config
    ]
    context = {
            "project_name": TODO,
            "short_name": TODO,
            "network": TODO,
            "network_test": TODO,       
        }
    

    for config in configs:
        context.update(config)

    if context['base']:
        context.update({
            "base_tld": TODO,
            "base_tld_hash": TODO,
        })
    protocol = {
        "default_context": context
    }

    with open(args.output, 'w') as f:
        yaml.dump(protocol, f, width=1<<256, sort_keys=False)

if __name__ == '__main__':
    main()

