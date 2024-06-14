import fs from 'fs-extra';
import fetch from 'node-fetch';


const baseHost = 'eth.blockscout.com';
const baseName = baseHost.replace(/\./g, '_');
const outputDir = './crate/tests/recorded/'
const paths = [
    "/api/v1/health",
    "/api/v2/blocks",
    "/api/v2/transactions",
    "/api/v2/transactions/0x4dd7e3f4522fcf2483ae422fd007492380051d87de6fdb17be71c7134e26857e/internal-transactions",
    "/api/v2/smart-contracts",
    "/api/v2/smart-contracts/0x8FD4596d4E7788a71F82dAf4119D069a84E7d3f3",
    "/api/v2/tokens",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances",
    "/api/v2/tokens/0xB87b96868644d99Cc70a8565BA7311482eDEBF6e/instances/1",
]


async function main(): Promise<any> {
    // const swaggerYaml = await fs.readFile(swaggerPath , 'utf8');
    // const swaggerData = yaml.load(swaggerYaml) as Swagger.SwaggerV3;

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