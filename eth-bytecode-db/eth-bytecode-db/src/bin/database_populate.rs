use entity::{sea_orm_active_enums::BytecodeType, sources};
use eth_bytecode_db::{
    create::{create_source, VerificationResult},
    search::{find_partial_match_contract, BytecodeRemote},
};
use sea_orm::{Database, DatabaseConnection, EntityTrait, PaginatorTrait};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db_url = std::env::var_os("DATABASE_URL")
        .map(|v| v.into_string().unwrap())
        .expect("no DATABASE_URL env");
    let db: DatabaseConnection = Database::connect(db_url).await.unwrap();
    let count = sources::Entity::find().count(&db).await.unwrap();
    if count < 10000 {
        let semaphore = Arc::new(Semaphore::new(10));
        let db = Arc::new(db);
        let mut join_handles = Vec::new();

        for i in 0..1000 {
            if i % 100 == 0 {
                println!("SAME CONTRACTS. task #{}", i);
            }

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db = db.clone();
            join_handles.push(tokio::spawn(async move {
                let res = push_contract(db, 1, 1).await;
                drop(permit);
                res
            }));
        }

        for id in 10..5020 {
            if id % 100 == 0 {
                println!("DIFFERENT SMALL CONTRACTS. task #{}", id);
            }

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db = db.clone();
            join_handles.push(tokio::spawn(async move {
                let res = push_contract(db, id, 1).await;
                drop(permit);
                res
            }));
        }

        for id in 10..5020 {
            if id % 100 == 0 {
                println!("DIFFERENT BIG CONTRACT. task #{}", id);
            }
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db = db.clone();
            join_handles.push(tokio::spawn(async move {
                let res = push_contract(db, id, 2).await;
                drop(permit);
                res
            }));
        }

        for handle in join_handles {
            handle.await.unwrap().unwrap();
        }
    } else {
        println!("database is full already. search");
        let n = 1;
        let now = std::time::Instant::now();
        for i in 0..n {
            let raw_creation_input = get_contract(91 + i, 1)
                .local_creation_input_parts
                .iter()
                .map(|p| p.data.trim_start_matches("0x"))
                .collect::<Vec<_>>()
                .join("");
            let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
                .unwrap()
                .0;
            let search = BytecodeRemote {
                data,
                bytecode_type: BytecodeType::CreationInput,
            };
            let partial_match = find_partial_match_contract(&db, search).await;
            println!("{:?}", partial_match);
        }
        println!("AVG time: {}", now.elapsed().as_secs_f64() / (n as f64));
    }
}

fn get_contract(id: i32, ty: i32) -> VerificationResult {
    match ty {
        1 => serde_json::from_str(&r#"
        {
            "file_name": "source.sol",
            "contract_name": "Id10$ID",
            "compiler_version": "v0.8.7+commit.e28d00a7",
            "evm_version": "london",
            "constructor_arguments": null,
            "optimization": false,
            "optimization_runs": null,
            "contract_libraries": {},
            "abi": "[{\"type\":\"function\",\"name\":\"get_id\",\"inputs\":[],\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"pure\"}]",
            "sources": {
                "source.sol": "// SPDX-License-Identifier: GPL-3.0\npragma solidity =0.8.7;\ncontract Id10$ID {\n    function get_id() public pure returns (uint) {\n        return 0x10$ID;\n    }\n}\n"
            },
            "compiler_settings": "{\"optimizer\":{\"enabled\":false},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"outputSelection\":{\"*\":{\"\":[\"ast\"],\"*\":[\"abi\",\"evm.bytecode\",\"evm.deployedBytecode\",\"evm.methodIdentifiers\"]}},\"evmVersion\":\"london\",\"libraries\":{}}",
            "local_creation_input_parts": [
                {
                    "type": "Main",
                    "data": "0x608060405234801561001057600080fd5b5060bb8061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063f43fa80514602d575b600080fd5b60336047565b604051603e91906062565b60405180910390f35b60006510$ID905090565b605c81607b565b82525050565b6000602082019050607560008301846055565b92915050565b600081905091905056fe"
                },
                {
                    "type": "Meta",
                    "data": "0xa2646970667358221220ad5a5e9ea0429c6665dc23af78b0acca8d56235be9dc3573672141811ea4a0da64736f6c63430008070033"
                }
            ],
            "local_deployed_bytecode_parts": [
                {
                    "type": "Main",
                    "data": "0x6080604052348015600f57600080fd5b506004361060285760003560e01c8063f43fa80514602d575b600080fd5b60336047565b604051603e91906062565b60405180910390f35b60006510$ID905090565b605c81607b565b82525050565b6000602082019050607560008301846055565b92915050565b600081905091905056fe"
                },
                {
                    "type": "Meta",
                    "data": "0xa2646970667358221220ad5a5e9ea0429c6665dc23af78b0acca8d56235be9dc3573672141811ea4a0da64736f6c63430008070033"
                }
            ]
        }"#.replace("$ID", &format!("{:0>10}", id)),
    ).unwrap(),
        2 => serde_json::from_str(&r#"
        {
            "file_name": "source.sol",
            "contract_name": "Id10$ID",
            "compiler_version": "v0.8.7+commit.e28d00a7",
            "evm_version": "london",
            "constructor_arguments": null,
            "optimization": false,
            "optimization_runs": null,
            "contract_libraries": {},
            "abi": "[{\"type\":\"function\",\"name\":\"add\",\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"x\",\"type\":\"uint256\"}],\"stateMutability\":\"pure\"},{\"type\":\"function\",\"name\":\"div\",\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"x\",\"type\":\"uint256\"}],\"stateMutability\":\"pure\"},{\"type\":\"function\",\"name\":\"get_id\",\"inputs\":[],\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"pure\"},{\"type\":\"function\",\"name\":\"mul\",\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"x\",\"type\":\"uint256\"}],\"stateMutability\":\"pure\"},{\"type\":\"function\",\"name\":\"sub\",\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"x\",\"type\":\"uint256\"}],\"stateMutability\":\"pure\"}]",
            "sources": {
                "source.sol": "// SPDX-License-Identifier: GPL-3.0\npragma solidity =0.8.7;\ncontract IdType2_10$ID {\n    function get_id() public pure returns (uint) {\n        return 0x10$ID;\n    }\n\n    function add(uint a, uint b) public pure returns (uint x) {\n        x = a + b;\n    }\n\n    function sub(uint a, uint b) public pure returns (uint x) {\n        x = a - b;\n    }\n\n    function mul(uint a, uint b) public pure returns (uint x) {\n        x = a * b;\n    }\n\n    function div(uint a, uint b) public pure returns (uint x) {\n        require(b > 0, \"oooooof\");\n\n        x = a / b;\n    }\n}\n"
            },
            "compiler_settings": "{\"optimizer\":{\"enabled\":false},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"outputSelection\":{\"*\":{\"\":[\"ast\"],\"*\":[\"abi\",\"evm.bytecode\",\"evm.deployedBytecode\",\"evm.methodIdentifiers\"]}},\"evmVersion\":\"london\",\"libraries\":{}}",
            "local_creation_input_parts": [
                {
                    "type": "Main",
                    "data": "0x608060405234801561001057600080fd5b506104ad806100206000396000f3fe608060405234801561001057600080fd5b50600436106100575760003560e01c8063771602f71461005c578063a391c15b1461008c578063b67d77c5146100bc578063c8a4ac9c146100ec578063f43fa8051461011c575b600080fd5b610076600480360381019061007191906101f7565b61013a565b6040516100839190610289565b60405180910390f35b6100a660048036038101906100a191906101f7565b610150565b6040516100b39190610289565b60405180910390f35b6100d660048036038101906100d191906101f7565b6101a8565b6040516100e39190610289565b60405180910390f35b610106600480360381019061010191906101f7565b6101be565b6040516101139190610289565b60405180910390f35b6101246101d4565b6040516101319190610289565b60405180910390f35b6000818361014891906102b5565b905092915050565b6000808211610194576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161018b90610269565b60405180910390fd5b81836101a0919061030b565b905092915050565b600081836101b69190610396565b905092915050565b600081836101cc919061033c565b905092915050565b60006510$ID905090565b6000813590506101f181610460565b92915050565b6000806040838503121561020e5761020d610432565b5b600061021c858286016101e2565b925050602061022d858286016101e2565b9150509250929050565b60006102446007836102a4565b915061024f82610437565b602082019050919050565b610263816103ca565b82525050565b6000602082019050818103600083015261028281610237565b9050919050565b600060208201905061029e600083018461025a565b92915050565b600082825260208201905092915050565b60006102c0826103ca565b91506102cb836103ca565b9250827fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff03821115610300576102ff6103d4565b5b828201905092915050565b6000610316826103ca565b9150610321836103ca565b92508261033157610330610403565b5b828204905092915050565b6000610347826103ca565b9150610352836103ca565b9250817fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff048311821515161561038b5761038a6103d4565b5b828202905092915050565b60006103a1826103ca565b91506103ac836103ca565b9250828210156103bf576103be6103d4565b5b828203905092915050565b6000819050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601260045260246000fd5b600080fd5b7f6f6f6f6f6f6f6600000000000000000000000000000000000000000000000000600082015250565b610469816103ca565b811461047457600080fd5b5056fe"
                },
                {
                    "type": "Meta",
                    "data": "0xa26469706673582212205b028181a8351ffe5cb4b09e29c981a2b08388f549307d52efc6ff2538c34f9564736f6c6343000807"
                }
            ],
            "local_deployed_bytecode_parts": [
                {
                    "type": "Main",
                    "data": "0x608060405234801561001057600080fd5b50600436106100575760003560e01c8063771602f71461005c578063a391c15b1461008c578063b67d77c5146100bc578063c8a4ac9c146100ec578063f43fa8051461011c575b600080fd5b610076600480360381019061007191906101f7565b61013a565b6040516100839190610289565b60405180910390f35b6100a660048036038101906100a191906101f7565b610150565b6040516100b39190610289565b60405180910390f35b6100d660048036038101906100d191906101f7565b6101a8565b6040516100e39190610289565b60405180910390f35b610106600480360381019061010191906101f7565b6101be565b6040516101139190610289565b60405180910390f35b6101246101d4565b6040516101319190610289565b60405180910390f35b6000818361014891906102b5565b905092915050565b6000808211610194576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161018b90610269565b60405180910390fd5b81836101a0919061030b565b905092915050565b600081836101b69190610396565b905092915050565b600081836101cc919061033c565b905092915050565b60006510$ID905090565b6000813590506101f181610460565b92915050565b6000806040838503121561020e5761020d610432565b5b600061021c858286016101e2565b925050602061022d858286016101e2565b9150509250929050565b60006102446007836102a4565b915061024f82610437565b602082019050919050565b610263816103ca565b82525050565b6000602082019050818103600083015261028281610237565b9050919050565b600060208201905061029e600083018461025a565b92915050565b600082825260208201905092915050565b60006102c0826103ca565b91506102cb836103ca565b9250827fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff03821115610300576102ff6103d4565b5b828201905092915050565b6000610316826103ca565b9150610321836103ca565b92508261033157610330610403565b5b828204905092915050565b6000610347826103ca565b9150610352836103ca565b9250817fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff048311821515161561038b5761038a6103d4565b5b828202905092915050565b60006103a1826103ca565b91506103ac836103ca565b9250828210156103bf576103be6103d4565b5b828203905092915050565b6000819050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601260045260246000fd5b600080fd5b7f6f6f6f6f6f6f6600000000000000000000000000000000000000000000000000600082015250565b610469816103ca565b811461047457600080fd5b5056fe"
                },
                {
                    "type": "Meta",
                    "data": "0xa26469706673582212205b028181a8351ffe5cb4b09e29c981a2b08388f549307d52efc6ff2538c34f9564736f6c6343000807"
                }
            ]
        }
        "#.replace("$ID", &format!("{:0>10}", id))
    ).unwrap(),

        _ => panic!("unknow type")
    }
}

async fn push_contract(db: Arc<DatabaseConnection>, id: i32, ty: i32) -> Result<(), anyhow::Error> {
    let verification_result = get_contract(id, ty);
    println!("push contract {}/{}", ty, id);
    create_source(db.as_ref(), verification_result).await?;
    Ok(())
}
