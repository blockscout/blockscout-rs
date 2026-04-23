import os
from enum import Enum
import json

OUTPUT_DIR = os.getenv("OUTPUT_DIR")

DATASET_DIR = os.getenv("DATASET")
if DATASET_DIR is None:
    print("Path to the smart-contract-fiesta dataset must be specified (env `DATASET=`)")
    exit(1)

ROOT_CONTRACTS_DIR = os.path.join(DATASET_DIR, "organized_contracts")
INDEX_FILE = os.path.join(DATASET_DIR, "address_bytecodehash_index")

class VerificationMethod(Enum):
    SOLIDITY_SINGLE = 1
    SOLIDITY_MULTIPLE = 2
    SOLIDITY_STANDARD = 3
    VYPER_SINGLE = 4

    def to_string(self):
        if self == VerificationMethod.SOLIDITY_SINGLE:
            return "solidity_single"
        elif self == VerificationMethod.SOLIDITY_MULTIPLE:
            return "solidity_multiple"
        elif self == VerificationMethod.SOLIDITY_STANDARD:
            return "solidity_standard"
        elif self == VerificationMethod.VYPER_SINGLE:
            return "vyper_single"
        else:
            assert(False, "unknown verification method")

def get_bytecode_hashes():
    bytecode_hashes = set()

    prefixes = os.listdir(ROOT_CONTRACTS_DIR)
    for prefix in prefixes:
        path = os.path.join(ROOT_CONTRACTS_DIR, prefix)
        for subdir in os.listdir(path):
            bytecode_hashes.add(subdir)

    return bytecode_hashes

def construct_verification_data(contract_address: str, bytecode_hash: 'str') -> (VerificationMethod, dict):
    directory = build_dir(bytecode_hash)
    verification_method = calculate_verification_method(directory)
    if verification_method == VerificationMethod.SOLIDITY_SINGLE:
        data = construct_solidity_single_data(contract_address, directory)
    elif verification_method == VerificationMethod.SOLIDITY_MULTIPLE:
        data = construct_solidity_multiple_data(contract_address, directory)
    elif verification_method == VerificationMethod.SOLIDITY_STANDARD:
        data = construct_solidity_standard_data(contract_address, directory)
    elif verification_method == VerificationMethod.VYPER_SINGLE:
        data = construct_vyper_single_data(contract_address, directory)
    else:
        assert(False, "unknown verification method")

    return verification_method, data

def build_dir(bytecode_hash: 'str') -> str:
    return os.path.join(ROOT_CONTRACTS_DIR, bytecode_hash[0:2], bytecode_hash)

def calculate_verification_method(directory: bytes) -> VerificationMethod:
    files = os.listdir(directory)
    if "main.vy" in files:
        return VerificationMethod.VYPER_SINGLE
    elif "contract.json" in files:
        return VerificationMethod.SOLIDITY_STANDARD
    elif len(files) > 2:
        return VerificationMethod.SOLIDITY_MULTIPLE
    else:
        return VerificationMethod.SOLIDITY_SINGLE

def construct_solidity_single_data(contract_address: str, directory: bytes) -> dict:
    with open(os.path.join(directory, "metadata.json"), 'r') as metadata_file:
        metadata = json.load(metadata_file)
    with open(os.path.join(directory, "main.sol"), 'r') as source_file:
        source = source_file.read()

    data = dict()
    data["contract_address"] = contract_address
    data["contract_name"] = metadata["ContractName"]
    data["compiler_version"] = metadata["CompilerVersion"]
    data["optimizations"] = metadata["OptimizationUsed"]
    data["optimization_runs"] = metadata["Runs"]
    data["source"] = source

    return data

def construct_solidity_multiple_data(contract_address: str, directory: bytes) -> dict:
    with open(os.path.join(directory, "metadata.json"), 'r') as metadata_file:
        metadata = json.load(metadata_file)

    sources = dict()
    for source_name in os.listdir(directory):
        if source_name == "metadata.json": continue
        with open(os.path.join(directory, source_name), 'r') as source_file:
            source = source_file.read()
            sources[source_name] = source

    data = dict()
    data["contract_address"] = contract_address
    data["contract_name"] = metadata["ContractName"]
    data["compiler_version"] = metadata["CompilerVersion"]
    data["optimizations"] = metadata["OptimizationUsed"]
    data["optimization_runs"] = metadata["Runs"]
    data["sources"] = sources

    return data

def construct_solidity_standard_data(contract_address: str, directory: bytes) -> dict:
    with open(os.path.join(directory, "metadata.json"), 'r') as metadata_file:
        metadata = json.load(metadata_file)

    with open(os.path.join(directory, "contract.json"), 'r') as standard_json_file:
        standard_json = json.load(standard_json_file)

    data = dict()
    data["contract_address"] = contract_address
    data["contract_name"] = metadata["ContractName"]
    data["compiler_version"] = metadata["CompilerVersion"]
    data["standard_json"] = standard_json

    return data

def construct_vyper_single_data(contract_address: str, directory: bytes) -> dict:
    with open(os.path.join(directory, "metadata.json"), 'r') as metadata_file:
        metadata = json.load(metadata_file)
    with open(os.path.join(directory, "main.vy"), 'r') as source_file:
        source = source_file.read()

    data = dict()
    data["contract_address"] = contract_address
    data["contract_name"] = metadata["ContractName"]
    data["compiler_version"] = metadata["CompilerVersion"]
    data["optimizations"] = metadata["OptimizationUsed"]
    data["optimization_runs"] = metadata["Runs"]
    data["source"] = source

    return data

def main():
    print("Prepare the fiesta dataset. Contracts left:")

    bytecode_hashes = get_bytecode_hashes()
    print(len(bytecode_hashes))

    output_dir = OUTPUT_DIR if OUTPUT_DIR is not None else "."
    results_dir = os.path.join(output_dir, "dataset")
    with open(INDEX_FILE, 'r') as index_file:
        for line in index_file:
            line = line.strip().split(':')
            contract_address, bytecode_hash = line[0], line[1]
            if bytecode_hash in bytecode_hashes:
                (verification_method, data) = construct_verification_data(contract_address, bytecode_hash)

                filename = os.path.join(results_dir, verification_method.to_string(), contract_address)
                os.makedirs(os.path.dirname(filename), exist_ok=True)
                with open(filename, 'w') as file:
                    file.write(json.dumps(data))

                bytecode_hashes.remove(bytecode_hash)

                if len(bytecode_hashes) % 10000 == 0:
                    print(len(bytecode_hashes))

if __name__ == '__main__':
    main()