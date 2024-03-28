import { Elysia, t } from "elysia";
import { swagger } from '@elysiajs/swagger'
import { whatsabi } from "@shazow/whatsabi";
import { ethers } from "ethers";
import { logger } from "@bogeychan/elysia-logger";

const port = process.env.WHATSABI__PORT;

const signatureLookup = new whatsabi.loaders.MultiSignatureLookup([
    new whatsabi.loaders.OpenChainSignatureLookup(),
    new whatsabi.loaders.FourByteSignatureLookup(),
]);

export function initApp(port: string | number) {
    return new Elysia()
        .use(swagger({
            path: '/api/v1/swagger'
        }))
        .use(
            logger({
                level: "info",
            })
        )
        .get("/api/v1/abi", async ( { request, log, query }) => {
            log.info(request, "New request");
            const result = await processAbi(query.address, query.provider);
            log.info(request, `Found ${result.length} abi items`);
            return result;
        }, {
            query: t.Object({
                address: t.String(),
                provider: t.String(),
            }),
            beforeHandle({ error, query }) {
                query.address = normalizeAddress(query.address);
                if (!ethers.isAddress(query.address)) {
                    return error(400, "Invalid address value")
                }
            }
        })
        .listen(port);
}

let app = initApp(port);
export type App = typeof app;

console.log(
    `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`
);

async function processAbi(address: string, provider_url: string) {
    const provider = ethers.getDefaultProvider(provider_url);

    let abi = await whatsabi.autoload(address, {
        provider: provider,
        signatureLookup: signatureLookup,
        abiLoader: false
    });

    return abi.abi
}

function normalizeAddress(address: string) {
    let normalized = address.toLowerCase();
    if (!normalized.startsWith('0x')) {
        normalized = '0x' + normalized;
    }
    return normalized;
}