
import argparse
import os
import requests
import json
from events import abi_from_str

def parse_arguments():
    parser = argparse.ArgumentParser(description="Get abi from etherscan")
    parser.add_argument(
        '-e', '--endpoint',
        required=True,
        help='Specify etherscan api endpoint with `apikey` query param provided'
    )

    
    parser.add_argument(
        '-a', '--addresses',
        required=True,
        type=str,
        help='Specify addresses of contracts'
    )

    # Add argument for 'output'
    parser.add_argument(
        '-o', '--output',
        default='output-abis.json',
        type=str,
        help='Specify the output file'
    )

    # Parse the command line arguments
    args = parser.parse_args()

    return args


def make_get_request(url, address):
    url = f"{url}&address={address}"
    response = requests.get(url)
    r = response.json()
    if r['status'] != '1':
        if r['message'] == 'NOTOK':
            print(f'skip {address} -- not verified')
            return
        else:
            raise Exception(f'invalid etherscan response: {r}')
        
    else:
        return r['result']

def get_abi(endpoint, address):
    endpoint = f"{endpoint}&module=contract&action=getabi"
    return make_get_request(endpoint, address)

def get_source_code(endpoint, address):
    endpoint = f"{endpoint}&module=contract&action=getsourcecode"
    return make_get_request(endpoint, address)



def main():
    args = parse_arguments()
    endpoint = args.endpoint
    addresses = args.addresses.split(',')
    result = {}
    for address in addresses:
        contract = get_source_code(endpoint, address)
        if not contract:
            print(f'contract {address} not found')
            continue
        contract = contract[0]
        contract_address = contract.get('Implementation') or address
        try:
            abi = get_abi(endpoint, contract_address)
            if not abi:
                print(f'contract {address} ({contract_address}) doesnt have abi')
                continue
            result[address] = abi_from_str(abi)
        except Exception as err:
            raise Exception(f'error for address "{address}": {err}')
    
    with open(args.output, 'w') as f:
        json.dump(result, f, indent=2)

if __name__ == '__main__':
    main()