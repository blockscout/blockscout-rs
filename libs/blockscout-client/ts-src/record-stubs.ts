import fs from 'fs-extra';
import fetch from 'node-fetch';


const baseHost = 'eth.blockscout.com';
const baseName = baseHost.replace(/\./g, '_');
const outputDir = './crate/tests/recorded/'
const paths = [
    "/api/health",
    // "/api/v1/health", // v1/health is legacy, dont record it
    "/api/v2/blocks",
    "/api/v2/transactions",
    "/api/v2/transactions/0x4dd7e3f4522fcf2483ae422fd007492380051d87de6fdb17be71c7134e26857e",
    "/api/v2/transactions/0x4dd7e3f4522fcf2483ae422fd007492380051d87de6fdb17be71c7134e26857e/internal-transactions",
    "/api/v2/smart-contracts",
    "/api/v2/smart-contracts/0x8FD4596d4E7788a71F82dAf4119D069a84E7d3f3",
    "/api/v2/tokens",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances/1",

    // Health endpoints
    "/api/health",
    
    // Blocks endpoints
    "/api/v2/blocks",
    "/api/v2/blocks/12345", // Example block number
    
    // Transactions endpoints
    "/api/v2/transactions",
    "/api/v2/transactions/0x4dd7e3f4522fcf2483ae422fd007492380051d87de6fdb17be71c7134e26857e",
    "/api/v2/transactions/0x4dd7e3f4522fcf2483ae422fd007492380051d87de6fdb17be71c7134e26857e/internal-transactions",
    
    // Smart Contracts endpoints
    "/api/v2/smart-contracts",
    // "/api/v2/smart-contracts?q=contract",
    // "/api/v2/smart-contracts?filter=verified",
    "/api/v2/smart-contracts/0x8FD4596d4E7788a71F82dAf4119D069a84E7d3f3",
    
    // Tokens endpoints
    "/api/v2/tokens",
    // "/api/v2/tokens?q=token",
    // "/api/v2/tokens?type=ERC-20",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances/1",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/counters",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/holders",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/transfers",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances/1/holders",
    
    // Token Transfers endpoints
    "/api/v2/token-transfers",
    
    // Addresses endpoints
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/blocks-validated",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/counters",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/logs",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/nft/collections",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/token-balances",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/token-transfers",
    "/api/v2/addresses/0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de/tokens",
    
    // Search endpoints
    // "/api/v2/search?q=USDT",
    
    // Stats endpoints
    "/api/v2/stats",
    
    // Config endpoints
    "/api/v2/config/json-rpc-url",
    
    // // Proxy endpoints
    // "/api/v2/proxy/account-abstraction/status",
    
    // // Celestia Service endpoints
    // "/api/v1/celestia/blob?height=123&commitment=commitment_value&skip_data=false",
    
]



async function main(): Promise<any> {
    for (const path of paths) {
        const url = `https://${baseHost}${path}`;
        console.log(`Making request to ${url}`);

        const response = await fetch(url);

        // write wiremock stubs
        const responseJson = await response.json();
        const stubs = {
            request: {
                method: 'GET',
                urlPath: path
            },
            response: {
                status: 200,
                body: JSON.stringify(responseJson),
                headers: {
                    'Content-Type': 'application/json'
                }
            }
        };
        const outputFileName = `${outputDir}/${baseName}/${path.replace(/\//g, '_')}.json`;
        await fs.ensureFile(outputFileName);
        await fs.writeFile(outputFileName, JSON.stringify(stubs, null, 2));
    }

}

main().catch(console.error);
