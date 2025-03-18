import { describe, expect, it } from 'bun:test'
import {getPort, initApp} from "../src";

const port = getPort();
const app = initApp(port);

describe('Processing abi requests', () => {
    async function makeRequest(address: string | null, provider: string | null) {
        let url = `http://localhost:${port}/api/v1/abi?`;
        if (address != null) {
            url = url.concat(`address=${address}&`);
        }
        if (provider != null) {
            url = url.concat(`provider=${provider}`);
        }
        return await app.handle(new Request(url))
    }

    it('returns an abi with blockscout as provider', async () => {
        const response = await makeRequest(
            "0xe8ef418ed75d744e2868c0d2f898c6a41bb17d6e",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(200);

        const expected = JSON.parse("[{\"type\":\"function\",\"selector\":\"0xeced3873\",\"sig\":\"publicSaleDate()\",\"name\":\"publicSaleDate\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xedec5f27\",\"sig\":\"whitelistUsers(address[])\",\"name\":\"whitelistUsers\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address[]\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xf2c4ce1e\",\"sig\":\"setNotRevealedURI(string)\",\"name\":\"setNotRevealedURI\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"string\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xf2fde38b\",\"sig\":\"transferOwnership(address)\",\"name\":\"transferOwnership\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xd0eb26b0\",\"sig\":\"setNftPerAddressLimit(uint256)\",\"name\":\"setNftPerAddressLimit\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xd5abeb01\",\"sig\":\"maxSupply()\",\"name\":\"maxSupply\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xda3ef23f\",\"sig\":\"setBaseExtension(string)\",\"name\":\"setBaseExtension\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"string\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xe985e9c5\",\"sig\":\"isApprovedForAll(address,address)\",\"name\":\"isApprovedForAll\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xba7d2c76\",\"sig\":\"nftPerAddressLimit()\",\"name\":\"nftPerAddressLimit\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xc6682862\",\"sig\":\"baseExtension()\",\"name\":\"baseExtension\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xc87b56dd\",\"sig\":\"tokenURI(uint256)\",\"name\":\"tokenURI\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xcc9ff9c6\",\"sig\":\"preSaleCost()\",\"name\":\"preSaleCost\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xa22cb465\",\"sig\":\"setApprovalForAll(address,bool)\",\"name\":\"setApprovalForAll\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"bool\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xa475b5dd\",\"sig\":\"reveal()\",\"name\":\"reveal\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xb88d4fde\",\"sig\":\"safeTransferFrom(address,address,uint256,bytes)\",\"name\":\"safeTransferFrom\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"address\",\"name\":\"\"},{\"type\":\"uint256\",\"name\":\"\"},{\"type\":\"bytes\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x8fdcf942\",\"sig\":\"setPresaleCost(uint256)\",\"name\":\"setPresaleCost\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x95d89b41\",\"sig\":\"symbol()\",\"name\":\"symbol\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0xa0712d68\",\"sig\":\"mint(uint256)\",\"name\":\"mint\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0xa18116f1\",\"sig\":\"preSaleMaxSupply()\",\"name\":\"preSaleMaxSupply\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x7f00c7a6\",\"sig\":\"setmaxMintAmount(uint256)\",\"name\":\"setmaxMintAmount\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x831e60de\",\"sig\":\"getCurrentCost()\",\"name\":\"getCurrentCost\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x83a076be\",\"sig\":\"gift(uint256,address)\",\"name\":\"gift\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"},{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x8da5cb5b\",\"sig\":\"owner()\",\"name\":\"owner\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x715018a6\",\"sig\":\"renounceOwnership()\",\"name\":\"renounceOwnership\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x743c7f6b\",\"sig\":\"setPreSaleDate(uint256)\",\"name\":\"setPreSaleDate\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x7967a50a\",\"sig\":\"preSaleEndDate()\",\"name\":\"preSaleEndDate\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x7effc032\",\"sig\":\"maxMintAmountPresale()\",\"name\":\"maxMintAmountPresale\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x6f9fb98a\",\"sig\":\"getContractBalance()\",\"name\":\"getContractBalance\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x70a08231\",\"sig\":\"balanceOf(address)\",\"name\":\"balanceOf\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x714c5398\",\"sig\":\"getBaseURI()\",\"name\":\"getBaseURI\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x5c975abb\",\"sig\":\"paused()\",\"name\":\"paused\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x6352211e\",\"sig\":\"ownerOf(uint256)\",\"name\":\"ownerOf\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x669736c0\",\"sig\":\"setmaxMintAmountPreSale(uint256)\",\"name\":\"setmaxMintAmountPreSale\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x6f8b44b0\",\"sig\":\"setMaxSupply(uint256)\",\"name\":\"setMaxSupply\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x44a0d68a\",\"sig\":\"setCost(uint256)\",\"name\":\"setCost\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x4f6ccce7\",\"sig\":\"tokenByIndex(uint256)\",\"name\":\"tokenByIndex\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x51830227\",\"sig\":\"revealed()\",\"name\":\"revealed\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x55f804b3\",\"sig\":\"setBaseURI(string)\",\"name\":\"setBaseURI\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"string\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x3ccfd60b\",\"sig\":\"withdraw()\",\"name\":\"withdraw\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x42842e0e\",\"sig\":\"safeTransferFrom(address,address,uint256)\",\"name\":\"safeTransferFrom\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"address\",\"name\":\"\"},{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x42f0ca0d\",\"sig\":\"setPreSaleEndDate(uint256)\",\"name\":\"setPreSaleEndDate\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x438b6300\",\"sig\":\"walletOfOwner(address)\",\"name\":\"walletOfOwner\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x2e09282e\",\"sig\":\"nftPerAddressLimitPresale()\",\"name\":\"nftPerAddressLimitPresale\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x2f745c59\",\"sig\":\"tokenOfOwnerByIndex(address,uint256)\",\"name\":\"tokenOfOwnerByIndex\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x3af32abf\",\"sig\":\"isWhitelisted(address)\",\"name\":\"isWhitelisted\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x18cae269\",\"sig\":\"addressMintedBalance(address)\",\"name\":\"addressMintedBalance\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x1985cc65\",\"sig\":\"preSaleDate()\",\"name\":\"preSaleDate\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x239c70ae\",\"sig\":\"maxMintAmount()\",\"name\":\"maxMintAmount\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x23b872dd\",\"sig\":\"transferFrom(address,address,uint256)\",\"name\":\"transferFrom\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"address\",\"name\":\"\"},{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x0a50716b\",\"sig\":\"setNftPerAddressLimitPreSale(uint256)\",\"name\":\"setNftPerAddressLimitPreSale\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x0e54a883\",\"sig\":\"setPublicSaleDate(uint256)\",\"name\":\"setPublicSaleDate\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x13faede6\",\"sig\":\"cost()\",\"name\":\"cost\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x18160ddd\",\"sig\":\"totalSupply()\",\"name\":\"totalSupply\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x081812fc\",\"sig\":\"getApproved(uint256)\",\"name\":\"getApproved\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x081c8c44\",\"sig\":\"notRevealedUri()\",\"name\":\"notRevealedUri\",\"constant\":false,\"payable\":false,\"inputs\":[]},{\"type\":\"function\",\"selector\":\"0x095ea7b3\",\"sig\":\"approve(address,uint256)\",\"name\":\"approve\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"address\",\"name\":\"\"},{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x0a403f04\",\"sig\":\"setPresaleMaxSupply(uint256)\",\"name\":\"setPresaleMaxSupply\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"uint256\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x01ffc9a7\",\"sig\":\"supportsInterface(bytes4)\",\"name\":\"supportsInterface\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"bytes4\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x02329a29\",\"sig\":\"pause(bool)\",\"name\":\"pause\",\"constant\":false,\"payable\":false,\"inputs\":[{\"type\":\"bool\",\"name\":\"\"}]},{\"type\":\"function\",\"selector\":\"0x06fdde03\",\"sig\":\"name()\",\"name\":\"name\",\"constant\":false,\"payable\":false,\"inputs\":[]}]")
        expect(await response.json()).toEqual(expected);
    })

    it('support different hex representations of address', async () => {
        // '0x' prefixed lowercase
        let response = await makeRequest(
            "0xe8ef418ed75d744e2868c0d2f898c6a41bb17d6e",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(200);

        // Without '0x' prefix
        response = await makeRequest(
            "e8ef418ed75d744e2868c0d2f898c6a41bb17d6e",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(200);

        // Random case letters also work
        response = await makeRequest(
            "0xe8EF418ED75d744e2868c0d2f898c6a41bb17d6e",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(200);

        // Invalid address returns 'Bad Request'
        response = await makeRequest(
            "0xcafe",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(400);
        expect(await response.text()).toContain("Invalid address");
    })

    it('returns an empty result if address is not a contract', async () => {
        // Address is an EOA
        let response = await makeRequest(
            "0xBE0eB53F46cd790Cd13851d5EFf43D12404d33E8",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(200);
        expect(await response.json()).toEqual([])

        // Address is not in the list of existing active addresses at all
        response = await makeRequest(
            "0x8438932513461375132897139713853424534523",
            "https://eth.blockscout.com/api/eth-rpc"
        );
        expect(response.status).toBe(200);
        expect(await response.json()).toEqual([])
    })
})