import requests
import re
import json

SLIP44_MSB = 0x80000000
ALL_COINS_URL = 'https://raw.githubusercontent.com/ensdomains/address-encoder/a2e171b59757a7444fceac0cc4d60a7d992c94bf/src/__tests__/index.test.ts'
ENCODING_URL = 'https://raw.githubusercontent.com/ensdomains/address-encoder/master/README.md'

# https://eips.ethereum.org/EIPS/eip-1191
EIP_1191_COIN_NAMES = {
    "RSK": 30
}

def convert_encoding(encoding, coin_name):
    maybe_chain_id = EIP_1191_COIN_NAMES.get(coin_name)
    return {
        "checksummed-hex": {"checkSummedHex": maybe_chain_id},
    }.get(encoding)

content = requests.get(ALL_COINS_URL).text
coins = list(re.finditer("name: '(\w+)',\s+coinType: (\d+|convertEVMChainIdToCoinType\((\d+)\)),", content))
print(f'found {len(coins)} coins')
result = {}
for r in coins:
    coin_name, coin_type = r.group(1), r.group(2)
    if coin_type.startswith('convertEVMChainIdToCoinType'):
        evm_coin_type = int(r.group(3))
        coin_type = SLIP44_MSB | evm_coin_type
    coin_type = str(coin_type)
    result[coin_name] = {
        "name": coin_name,
        "coinType": coin_type,
    }

content = requests.get(ENCODING_URL).text
encodings = list(re.finditer("- (\w+) \((.+)\)", content))
print(f'found {len(coins)} encodings')

for r in encodings:
    coin_name, encoding = r.group(1), r.group(2)
    encoding = convert_encoding(encoding, coin_name)
    
    maybe_coin = result.get(coin_name)
    if maybe_coin:
        maybe_coin["encoding"] = encoding

with open('coin_types.json', 'w') as f:
    json.dump(list(result.values()), f, indent=2)
