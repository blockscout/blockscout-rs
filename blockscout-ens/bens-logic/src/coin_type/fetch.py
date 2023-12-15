import requests
import re
import json

SLIP44_MSB = 0x80000000
url = 'https://raw.githubusercontent.com/ensdomains/address-encoder/a2e171b59757a7444fceac0cc4d60a7d992c94bf/src/__tests__/index.test.ts'


content = requests.get(url).text
coins = list(re.finditer("name: '(\w+)',\s+coinType: (\d+|convertEVMChainIdToCoinType\((\d+)\)),", content))
print(f'found {len(coins)} coins')
result = []
for r in coins:
    coin_name, coin_type = r.group(1), r.group(2)
    if coin_type.startswith('convertEVMChainIdToCoinType'):
        evm_coin_type = int(r.group(3))
        coin_type = SLIP44_MSB | evm_coin_type
    coin_type = str(coin_type)
    
    result.append({
        "name": coin_name,
        "coinType": coin_type,
    })

with open('coin_types.json', 'w') as f:
    json.dump(result, f, indent=2)