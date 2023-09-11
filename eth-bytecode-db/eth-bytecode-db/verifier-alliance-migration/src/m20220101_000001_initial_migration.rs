use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            -- Needed for gen_random_uuid()
            CREATE EXTENSION pgcrypto;

            -- The `code` table stores a mapping from code hash to bytecode
            -- This table may store both normalized and unnormalized code. Code is normalized when all
            -- libraries/immutable variables that are not constants are replaced with zeroes. In other words
            -- the variable `address private immutable FACTORY = 0xAABB...EEFF` would not be replaced with
            -- zeroes, but the variable `address private immutable OWNER = msg.sender` would.
            CREATE TABLE code
            (
                code_hash bytea PRIMARY KEY, -- the keccak256 hash of the bytecode
                code      bytea -- the raw bytecode itself
            );

            -- Used to designate non-existant code, which is different from empty code i.e. keccak256('')
            INSERT INTO code (code_hash, code) VALUES ('\x', NULL);

            -- The `contracts` table stores information which can be used to identify a unique contract in a
            -- chain-agnostic manner. In other words, suppose you deploy the same contract on two chains, all
            -- properties that would be shared across the two chains should go in this table because they uniquely
            -- identify the contract.
            CREATE TABLE contracts
            (
                id uuid PRIMARY KEY DEFAULT gen_random_uuid(),

                -- the creation code is the bytecode from the calldata (for eoa deployments) or given to create/create2
                -- the runtime code is the bytecode that was returned from the constructor
                -- both fields are not normalized

                creation_code_hash bytea NOT NULL REFERENCES code (code_hash),
                runtime_code_hash  bytea NOT NULL REFERENCES code (code_hash),

                CONSTRAINT contracts_pseudo_pkey UNIQUE (creation_code_hash, runtime_code_hash)
            );

            CREATE INDEX contracts_creation_code_hash ON contracts USING btree(creation_code_hash);
            CREATE INDEX contracts_runtime_code_hash ON contracts USING btree(runtime_code_hash);
            CREATE INDEX contracts_creation_code_hash_runtime_code_hash ON contracts USING btree(creation_code_hash, runtime_code_hash);

            -- The `contract_deployments` table stores information about a specific deployment unique to a chain.
            -- One contract address may have multiple deployments on a single chain if SELFDESTRUCT/CREATE2 is used
            -- The info stored in this table should be retrievable from an archive node. In other words, it should
            -- not be augmented with any inferred data
            CREATE TABLE contract_deployments
            (
                -- an opaque id assigned to this specific deployment, since it's easier to reference than the three fields below
                id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),

                -- these three fields uniquely identify a specific deployment, assuming that it is never possible
                -- to deploy to successfully an address twice in the same transaction
                -- (create2 -> selfdestruct -> create2 should revert on the second create2)
                -- in the case of a "genesis" contract, the transaction_hash should be set
                -- to keccak256(creation_code_hash || runtime_code_hash)
                chain_id           numeric NOT NULL,
                address            bytea NOT NULL,
                transaction_hash   bytea NOT NULL,

                -- geth full nodes have the option to prune the transaction index, so this is another way
                -- to find the transaction. be sure to check that the hash is correct!
                block_number       numeric,
                txindex            numeric,

                -- this is the address which actually deployed the contract (i.e. called the create/create2 opcode)
                deployer           bytea,

                -- the contract itself
                contract_id uuid NOT NULL REFERENCES contracts(id),

                CONSTRAINT contract_deployments_pseudo_pkey UNIQUE (chain_id, address, transaction_hash)
            );

            CREATE INDEX contract_deployments_contract_id ON contract_deployments USING btree(contract_id);

            -- The compiled_contracts table stores information about a specific compilation. A compilation is
            -- defined as a set of inputs (compiler settings, source code, etc) which uniquely correspond to a
            -- set of outputs (bytecode, documentation, ast, etc)
            CREATE TABLE compiled_contracts
            (
                -- an opaque id
                id                      uuid PRIMARY KEY DEFAULT gen_random_uuid(),

                -- these three fields uniquely identify the high-level compiler mode to use
                -- note that the compiler is the software ('solc', 'vyper', 'huff') while language is
                -- the syntax ('solidity', 'vyper', 'yul'). there may be future compilers which aren't solc
                -- but can still compile solidity, which is why we need to differentiate the two.
                -- the version should uniquely identify the compiler
                compiler                VARCHAR NOT NULL,
                version                 VARCHAR NOT NULL,
                language                VARCHAR NOT NULL,

                -- the name is arbitrary and often not a factor in verifying contracts (solidity encodes it in
                -- the auxdata which we ignore, and vyper doesn't even have the concept of sourceunit-level
                -- names)
                -- because of this we don't include it in the unique constraint
                name                    VARCHAR NOT NULL,

                -- the fully qualified name is compiler-specific and indicates exactly which contract to look for
                fully_qualified_name    VARCHAR NOT NULL,

                -- map of path to source code (string => string)
                sources                 jsonb NOT NULL,

                -- compiler-specific settings such as optimization, linking, etc (string => any)
                compiler_settings       jsonb NOT NULL,

                -- general and compiler-specific artifacts (abi, userdoc, devdoc, licenses, etc)
                compilation_artifacts   jsonb NOT NULL,

                -- note that we can't pull out creation/runtime code into its own table
                -- imagine that a future compiler and language combo result in the same bytecode
                -- this is something that we would want a record of, because the two sources are semantically
                -- unique
                -- in other words, the hypothetical table would need to be keyed on everything that this table already is

                -- these fields store info about the creation code (sourcemaps, linkreferences)
                creation_code_hash      bytea NOT NULL REFERENCES code (code_hash),
                creation_code_artifacts jsonb NOT NULL,

                -- these fields store info about the runtime code (sourcemaps, linkreferences, immutables)
                -- the runtime code should be normalized (i.e. immutables set to zero)
                runtime_code_hash       bytea NOT NULL REFERENCES code (code_hash),
                runtime_code_artifacts  jsonb NOT NULL,

                -- two different compilers producing the same bytecode is unique enough that we want to preserve it
                -- the same compiler with two different versions producing the same bytecode is not unique (f.ex nightlies)
                CONSTRAINT compiled_contracts_pseudo_pkey UNIQUE (compiler, language, creation_code_hash, runtime_code_hash)
            );

            CREATE INDEX compiled_contracts_creation_code_hash ON compiled_contracts USING btree (creation_code_hash);
            CREATE INDEX compiled_contracts_runtime_code_hash ON compiled_contracts USING btree (runtime_code_hash);

            -- The verified_contracts table links an on-chain contract with a compiled_contract
            -- Note that only one of creation or runtime bytecode must match, because:
            --   We could get a creation match but runtime mismatch if the contract is a proxy that uses assembly to return custom runtime bytecode
            --   We could get a runtime match but creation mismatch if the contract is deployed via a create2 factory
            CREATE TABLE verified_contracts
            (
                -- an opaque id
                id                       uuid PRIMARY KEY DEFAULT gen_random_uuid(),

                -- the foreign references
                compilation_id           uuid NOT NULL REFERENCES compiled_contracts (id),
                contract_id              uuid NOT NULL REFERENCES contracts(id),

                -- if the code matches, then the values and transformation fields contain
                -- all the information required to transform the compiled bytecode to the deployed bytecode
                -- see the json schemas provided for more information

                creation_match           bool NOT NULL,
                creation_values          jsonb,
                creation_transformations jsonb,

                runtime_match            bool NOT NULL,
                runtime_values           jsonb,
                runtime_transformations  jsonb,

                CONSTRAINT verified_contracts_pseudo_pkey UNIQUE (compilation_id, contract_id)
            );

            CREATE INDEX verified_contracts_contract_id ON verified_contracts USING btree (contract_id);
            CREATE INDEX verified_contracts_compilation_id ON verified_contracts USING btree (compilation_id);
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "verified_contracts";
            DROP TABLE "compiled_contracts";
            DROP TABLE "contract_deployments";
            DROP TABLE "contracts";
            DROP TABLE "code";

            DROP EXTENSION "pgcrypto";
        "#;

        crate::from_sql(manager, sql).await
    }
}
