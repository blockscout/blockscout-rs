/* Needed for gen_random_uuid() and digest(..) */
CREATE EXTENSION pgcrypto;

/*
    The `code` table stores a mapping from code hash to bytecode. This table may store
    both normalized and unnormalized code.

    Code is normalized when all libraries/immutable variables that are not constants are
    replaced with zeroes. In other words the variable `address private immutable FACTORY = 0xAABB...EEFF;`
    would not be replaced with zeroes, but the variable `address private immutable OWNER = msg.sender` would.

    The `code` column is not marked NOT NULL because we need to distinguish between
    empty code, and no code. Empty code occurs when a contract is deployed with no runtime code.
    No code occurs when a contract's code is written directly to the chain in a hard fork
*/
CREATE TABLE code
(
    /* the sha256 hash of the `code` column */
    code_hash   bytea NOT NULL PRIMARY KEY,

    /* timestamps */
    created_at  timestamptz NOT NULL DEFAULT NOW(),
    updated_at  timestamptz NOT NULL DEFAULT NOW(),

    /* ownership */
    created_by  varchar NOT NULL DEFAULT (current_user),
    updated_by  varchar NOT NULL DEFAULT (current_user),

    /*
        the keccak256 hash of the `code` column

        can be useful for lookups, as keccak256 is more common for Ethereum
        but we cannot use it as a primary key because postgres does not support the keccak256, and
        we cannot guarantee at the database level that provided value is the correct `code` hash
    */
    code_hash_keccak bytea NOT NULL,

    /* the bytecode */
    code    bytea

        CONSTRAINT code_hash_check
            CHECK (code IS NOT NULL and code_hash = digest(code, 'sha256') or code IS NULL and code_hash = '\x'::bytea)
    );

CREATE INDEX code_code_hash_keccak ON code USING btree(code_hash_keccak);

/* ensure the sentinel value exists */
INSERT INTO code (code_hash, code_hash_keccak, code) VALUES ('\x', '\x', NULL);

/*
    The `contracts` table stores information which can be used to identify a unique contract in a
    chain-agnostic manner. In other words, suppose you deploy the same contract on two chains, all
    properties that would be shared across the two chains should go in this table because they uniquely
    identify the contract.
*/
CREATE TABLE contracts
(
    /* an opaque id */
    id  uuid NOT NULL PRIMARY KEY DEFAULT gen_random_uuid(),

    /* timestamps */
    created_at  timestamptz NOT NULL DEFAULT NOW(),
    updated_at  timestamptz NOT NULL DEFAULT NOW(),

    /* ownership */
    created_by  varchar NOT NULL DEFAULT (current_user),
    updated_by  varchar NOT NULL DEFAULT (current_user),

    /*
        the creation code is the calldata (for eoa creations) or the instruction input (for create/create2)
        the runtime code is the bytecode that's returned by the creation code and stored on-chain

        neither fields are normalized
    */
    creation_code_hash  bytea NOT NULL REFERENCES code (code_hash),
    runtime_code_hash   bytea NOT NULL REFERENCES code (code_hash),

    CONSTRAINT contracts_pseudo_pkey UNIQUE (creation_code_hash, runtime_code_hash)
);

CREATE INDEX contracts_creation_code_hash ON contracts USING btree(creation_code_hash);
CREATE INDEX contracts_runtime_code_hash ON contracts USING btree(runtime_code_hash);
CREATE INDEX contracts_creation_code_hash_runtime_code_hash ON contracts USING btree(creation_code_hash, runtime_code_hash);

/*
    The `contract_deployments` table stores information about a specific deployment unique to a chain.
    One contract address may have multiple deployments on a single chain if SELFDESTRUCT/CREATE2 is used
    The info stored in this table should be retrievable from an archive node. In other words, it should
    not be augmented with any inferred data
*/
CREATE TABLE contract_deployments
(
    /* an opaque id*/
    id  uuid NOT NULL PRIMARY KEY DEFAULT gen_random_uuid(),

    /* timestamps */
    created_at  timestamptz NOT NULL DEFAULT NOW(),
    updated_at  timestamptz NOT NULL DEFAULT NOW(),

    /* ownership */
    created_by  varchar NOT NULL DEFAULT (current_user),
    updated_by  varchar NOT NULL DEFAULT (current_user),

    /*
        these three fields uniquely identify a specific deployment of a contract, assuming
        that it is impossible to deploy to successfully an address twice in the same transaction
        (create2 -> selfdestruct -> create2 should revert on the second create2)

        in the case of a "genesis" contract, the transaction_hash should be set
        to keccak256(creation_code_hash || runtime_code_hash). this is because the transaction_hash
        needs to differ to distinguish between two versions of the same genesis contract, and so
        it needs to embed inside it the only feature that changes.

        also note that for genesis contracts, creation_code_hash may be '\x' (i.e. there is no creation code)
    */
    chain_id            bigint NOT NULL, /* see https://github.com/verifier-alliance/database-specs/issues/23 */
    address             bytea NOT NULL,
    transaction_hash    bytea NOT NULL,

    /*
        geth full nodes have the ability to prune the transaction index, so if the transaction_hash
        can't be found directly, use the block_number and transaction_index. make sure to compare the transaction_hash to
        make sure it matches!

        for genesis contracts, both values should be set to -1
    */
    block_number        numeric NOT NULL,
    transaction_index   numeric NOT NULL,

    /*
        this is the address which actually deployed the contract (i.e. called the create/create2 opcode)
    */
    deployer    bytea NOT NULL,

    /* the contract itself */
    contract_id uuid NOT NULL REFERENCES contracts(id),

    CONSTRAINT contract_deployments_pseudo_pkey UNIQUE (chain_id, address, transaction_hash)
);

CREATE INDEX contract_deployments_contract_id ON contract_deployments USING btree(contract_id);

/*
    The `compiled_contracts` table stores information about a specific compilation. A compilation is
    defined as a set of inputs (compiler settings, source code, etc) which uniquely correspond to a
    set of outputs (bytecode, documentation, ast, etc)
*/
CREATE TABLE compiled_contracts
(
    /* an opaque id */
    id  uuid NOT NULL PRIMARY KEY DEFAULT gen_random_uuid(),

    /* timestamps */
    created_at  timestamptz NOT NULL DEFAULT NOW(),
    updated_at  timestamptz NOT NULL DEFAULT NOW(),

    /* ownership */
    created_by  varchar NOT NULL DEFAULT (current_user),
    updated_by  varchar NOT NULL DEFAULT (current_user),

    /*
        these three fields uniquely identify the high-level compiler mode to use

        note that the compiler is the software ('solc', 'vyper', 'huff') while language is
        the syntax ('solidity', 'vyper', 'yul'). there may be future compilers which aren't solc
        but can still compile solidity, which is why we need to differentiate the two

        the version should uniquely identify the compiler
    */
    compiler    VARCHAR NOT NULL,
    version     VARCHAR NOT NULL,
    language    VARCHAR NOT NULL,

    /*
        the name is arbitrary and often not a factor in verifying contracts (solidity encodes it in
        the auxdata which we ignore, and vyper doesn't even have the concept of sourceunit-level names)
        because of this we don't include it in the unique constraint. it is stored purely for informational
        purposes
    */
    name    VARCHAR NOT NULL,

    /* the fully qualified name is compiler-specific and indicates exactly which contract to look for */
    fully_qualified_name    VARCHAR NOT NULL,

    /* compiler-specific settings such as optimization, linking, etc (string => any) */
    compiler_settings       jsonb NOT NULL,

    /* general and compiler-specific artifacts (abi, userdoc, devdoc, licenses, etc) */
    compilation_artifacts   jsonb NOT NULL,

    /*
        note that we can't pull out creation/runtime code into its own table
        imagine that a future compiler and language combo result in the same bytecode
        this is something that we would want a record of, because the two sources are semantically
        unique
        in other words, the hypothetical table would need to be keyed on everything that this table already is
    */

    /* these fields store info about the creation code (sourcemaps, linkreferences) */
    creation_code_hash      bytea NOT NULL REFERENCES code (code_hash),
    creation_code_artifacts jsonb NOT NULL,

    /*
        these fields store info about the runtime code (sourcemaps, linkreferences, immutables)
        the runtime code should be normalized (i.e. immutables set to zero)
    */
    runtime_code_hash       bytea NOT NULL REFERENCES code (code_hash),
    runtime_code_artifacts  jsonb NOT NULL,

    /*
        two different compilers producing the same bytecode is unique enough that we want to preserve it
        the same compiler with two different versions producing the same bytecode is not unique (f.ex nightlies)
    */
    CONSTRAINT compiled_contracts_pseudo_pkey UNIQUE (compiler, language, creation_code_hash, runtime_code_hash)
);

CREATE INDEX compiled_contracts_creation_code_hash ON compiled_contracts USING btree (creation_code_hash);
CREATE INDEX compiled_contracts_runtime_code_hash ON compiled_contracts USING btree (runtime_code_hash);

/*
    The `sources` table stores the source code related to the contracts.
    It includes hashes of the source code and the code content itself.
*/
CREATE TABLE sources
(
    /* the sha256 hash of the source code */
    source_hash bytea NOT NULL PRIMARY KEY,

    /* the keccak256 hash of the source code */
    source_hash_keccak bytea NOT NULL,

    /* the actual source code content */
    content varchar NOT NULL,

    /* timestamps */
    created_at  timestamptz NOT NULL DEFAULT NOW(),
    updated_at  timestamptz NOT NULL DEFAULT NOW(),

    /* ownership */
    created_by  varchar NOT NULL DEFAULT (current_user),
    updated_by  varchar NOT NULL DEFAULT (current_user),

    CONSTRAINT source_hash_check CHECK (source_hash = digest(content, 'sha256'))
);

/*
    The `compiled_contracts_sources` table links a compiled_contract to its associated source files.
    This table contains a unique combination of compilation_id and path.
*/
CREATE TABLE compiled_contracts_sources
(
    id uuid NOT NULL PRIMARY KEY DEFAULT gen_random_uuid(),

    /* the specific compilation and the specific source */
    compilation_id uuid NOT NULL REFERENCES compiled_contracts(id),
    source_hash bytea NOT NULL REFERENCES sources(source_hash),

    /* the file path associated with this source code in the compilation */
    path varchar NOT NULL,

    CONSTRAINT compiled_contracts_sources_pseudo_pkey UNIQUE (compilation_id, path)
);

CREATE INDEX compiled_contracts_sources_source_hash ON compiled_contracts_sources USING btree (source_hash);
CREATE INDEX compiled_contracts_sources_compilation_id ON compiled_contracts_sources (compilation_id);

/*
    The verified_contracts table links an on-chain contract with a compiled_contract
    Note that only one of creation or runtime bytecode must match, because:
        We could get a creation match but runtime mismatch if the contract is a proxy that uses assembly to return custom runtime bytecode
        We could get a runtime match but creation mismatch if the contract is deployed via a create2 factory
*/
CREATE TABLE verified_contracts
(
    /* an opaque id, but sequentially ordered */
    id  BIGSERIAL NOT NULL PRIMARY KEY,

    /* timestamps */
    created_at  timestamptz NOT NULL DEFAULT NOW(),
    updated_at  timestamptz NOT NULL DEFAULT NOW(),

    /* ownership */
    created_by  varchar NOT NULL DEFAULT (current_user),
    updated_by  varchar NOT NULL DEFAULT (current_user),

    /* the specific deployment and the specific compilation */
    deployment_id   uuid NOT NULL REFERENCES contract_deployments (id),
    compilation_id  uuid NOT NULL REFERENCES compiled_contracts (id),

    /*
        if the code matches, then the values and transformation fields contain
        all the information required to transform the compiled bytecode to the deployed bytecode
        see the json schemas provided for more information
    */

    creation_match              bool NOT NULL,
    creation_values             jsonb,
    creation_transformations    jsonb,
    creation_metadata_match     bool,

    runtime_match           bool NOT NULL,
    runtime_values          jsonb,
    runtime_transformations jsonb,
    runtime_metadata_match  bool,

    CONSTRAINT verified_contracts_pseudo_pkey UNIQUE (compilation_id, deployment_id),

    CONSTRAINT verified_contracts_match_exists
        CHECK (creation_match = true OR runtime_match = true),
    CONSTRAINT verified_contracts_creation_match_integrity
        CHECK ((creation_match = false AND creation_values IS NULL AND creation_transformations IS NULL AND creation_metadata_match IS NULL) OR
               (creation_match = true AND creation_values IS NOT NULL AND creation_transformations IS NOT NULL AND creation_metadata_match IS NOT NULL)),
    CONSTRAINT verified_contracts_runtime_match_integrity
        CHECK ((runtime_match = false AND runtime_values IS NULL AND runtime_transformations IS NULL AND runtime_metadata_match IS NULL) OR
               (runtime_match = true AND runtime_values IS NOT NULL AND runtime_transformations IS NOT NULL AND runtime_metadata_match IS NOT NULL))
);

CREATE INDEX verified_contracts_deployment_id ON verified_contracts USING btree (deployment_id);
CREATE INDEX verified_contracts_compilation_id ON verified_contracts USING btree (compilation_id);

/*
    Helper functions used to ensure the correctness of json objects.
*/
CREATE OR REPLACE FUNCTION is_jsonb_object(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    jsonb_typeof(obj) = 'object';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION is_jsonb_string(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    jsonb_typeof(obj) = 'string';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION is_jsonb_array(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    jsonb_typeof(obj) = 'array';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION is_jsonb_number(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    jsonb_typeof(obj) = 'number';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION is_jsonb_null(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    jsonb_typeof(obj) = 'null';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION is_valid_hex(val text, repetition text)
    RETURNS boolean AS
$$
BEGIN
RETURN val SIMILAR TO CONCAT('0x([0-9|a-f|A-F][0-9|a-f|A-F])', repetition);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_json_object_keys(obj jsonb, mandatory_keys text[], optional_keys text[])
    RETURNS boolean AS
$$
BEGIN
RETURN
    -- ensures that all keys on the right exist as keys inside obj
    obj ?& mandatory_keys AND
        -- check that no unknown key exists inside obj
        bool_and(obj_keys = any (mandatory_keys || optional_keys))
        from (select obj_keys from jsonb_object_keys(obj) as obj_keys) as subquery;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_compilation_artifacts_abi(abi jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN is_jsonb_null(abi) OR is_jsonb_array(abi);
END;
$$ LANGUAGE plpgsql;

/*
   Validates the internal values of compilation_artifacts->'sources'.
   Precondition: sources MUST be a jsonb object.
*/
CREATE OR REPLACE FUNCTION validate_compilation_artifacts_sources_internal(obj jsonb)
    RETURNS boolean AS
$$
DECLARE
are_object_values_valid bool;
    are_ids_unique          bool;
BEGIN
SELECT bool_and (
           -- file name must be non-empty string
               length(key) > 0 AND
                   -- the corresponding value is expected to be an object with only the 'id' key
               is_jsonb_object(value) AND
               validate_json_object_keys(value, array ['id'], array []::text[]) AND
                   -- the value of 'id' key is expected to be a non-negative integer
               is_jsonb_number(value -> 'id') AND
               (value->>'id')::int >= 0
       )
INTO are_object_values_valid
FROM jsonb_each(obj);

SELECT count(value -> 'id') = count(DISTINCT value -> 'id')
INTO are_ids_unique
FROM jsonb_each(obj);

RETURN are_object_values_valid AND are_ids_unique;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_compilation_artifacts_sources(sources jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN is_jsonb_null(sources) OR (
    is_jsonb_object(sources) AND
    validate_compilation_artifacts_sources_internal(sources)
    );
END;
$$ LANGUAGE plpgsql;

/*
    Validation functions to be used in `compiled_contracts` artifact constraints.
*/
CREATE OR REPLACE FUNCTION validate_compilation_artifacts(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_object(obj) AND
    validate_json_object_keys(
            obj,
            array ['abi', 'userdoc', 'devdoc', 'sources', 'storageLayout'],
            array []::text[]
    ) AND
    validate_compilation_artifacts_abi(obj -> 'abi') AND
    validate_compilation_artifacts_sources(obj -> 'sources');
END;
$$ LANGUAGE plpgsql;

/*
   Validates the internal values of (creation/runtime)_code_artifacts->'cborAuxdata'.
   Precondition: obj MUST be a jsonb object.
*/
CREATE OR REPLACE FUNCTION validate_code_artifacts_cbor_auxdata_internal(obj jsonb)
    RETURNS boolean AS
$$
DECLARE
are_object_values_valid bool;
BEGIN
SELECT bool_and (
           -- file name must be non-empty string
               length(key) > 0 AND
                   -- the corresponding value is expected to be an object with only 'value' and 'offset' keys
               is_jsonb_object(value) AND
               validate_json_object_keys(value, array ['value', 'offset'], array []::text[]) AND
                   -- the value of 'value' key is expected to be a non-empty hex string
               is_jsonb_string(value -> 'value') AND
               is_valid_hex(value ->> 'value', '+') AND
                   -- the value of 'offset' key is expected to be a non-negative integer
               is_jsonb_number(value -> 'offset') AND
               (value->>'offset')::int >= 0
       )
INTO are_object_values_valid
FROM jsonb_each(obj);

RETURN are_object_values_valid;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_code_artifacts_cbor_auxdata(cbor_auxdata jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN is_jsonb_null(cbor_auxdata) OR (
    is_jsonb_object(cbor_auxdata) AND
    validate_code_artifacts_cbor_auxdata_internal(cbor_auxdata)
    );
END;
$$ LANGUAGE plpgsql;

/*
   Validates the content of (creation/runtime)_code_artifacts->'linkReferences'->'{file_name}'.
   Precondition: obj MUST be a jsonb object.
*/
CREATE OR REPLACE FUNCTION validate_code_artifacts_link_references_internal_file_libraries(obj jsonb)
    RETURNS boolean AS
$$
DECLARE
are_file_libraries_valid bool;
BEGIN
SELECT bool_and (
           -- library name must be non-empty string
               length(key) > 0 AND
                   -- the corresponding value is expected to be an array of objects
               is_jsonb_array(value) AND (
                   SELECT bool_and (
                                  is_jsonb_object(library_references) AND
                                      -- expected only 'start' (non-negative number) and 'length' (number always equals to 20) key-values
                                  validate_json_object_keys(library_references, array ['start', 'length'], array []::text[]) AND
                                  is_jsonb_number(library_references->'start') AND
                                  (library_references->'start')::int >= 0 AND
                is_jsonb_number(library_references->'length') AND
                (library_references->'length')::int = 20
                          )
                   FROM jsonb_array_elements(value) library_references
               )
       )
INTO are_file_libraries_valid
FROM jsonb_each(obj);

RETURN are_file_libraries_valid;
END;
$$ LANGUAGE plpgsql;

/*
   Validates the internal values of (creation/runtime)_code_artifacts->'linkReferences'.
   Precondition: obj MUST be a jsonb object.
*/
CREATE OR REPLACE FUNCTION validate_code_artifacts_link_references_internal(obj jsonb)
    RETURNS boolean AS
$$
DECLARE
are_file_levels_valid bool;
BEGIN
SELECT bool_and (
           -- file name must be non-empty string
               length(key) > 0 AND
                   -- the corresponding value is expected to be an object with library names as keys
               is_jsonb_object(value) AND
               validate_code_artifacts_link_references_internal_file_libraries(value)
       )
INTO are_file_levels_valid
FROM jsonb_each(obj);

RETURN are_file_levels_valid;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_code_artifacts_link_references(link_references jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN is_jsonb_null(link_references) OR (
    is_jsonb_object(link_references) AND
    validate_code_artifacts_link_references_internal(link_references)
    );
END;
$$ LANGUAGE plpgsql;

/*
   Validates the internal values of runtime_code_artifacts->'immutableReferences'.
   Precondition: obj MUST be a jsonb object.
*/
CREATE OR REPLACE FUNCTION validate_code_artifacts_immutable_references_internal(obj jsonb)
    RETURNS boolean AS
$$
DECLARE
are_values_valid bool;
BEGIN
SELECT bool_and (
           -- id must be non-empty string
               length(key) > 0 AND
                   -- the corresponding value is expected to be an array of objects
               is_jsonb_array(value) AND (
                   SELECT bool_and (
                                  is_jsonb_object(_references) AND
                                      -- expected only 'start' (non-negative number) and 'length' (positive number) key-values
                                  validate_json_object_keys(_references, array ['start', 'length'], array []::text[]) AND
                                  is_jsonb_number(_references->'start') AND
                                  (_references->'start')::int >= 0 AND
                is_jsonb_number(_references->'length') AND
                (_references->'length')::int > 0
                          )
                   FROM jsonb_array_elements(value) _references
               )
       )
INTO are_values_valid
FROM jsonb_each(obj);

RETURN are_values_valid;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_code_artifacts_immutable_references(immutable_references jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN is_jsonb_null(immutable_references) OR (
    is_jsonb_object(immutable_references) AND
    validate_code_artifacts_immutable_references_internal(immutable_references)
    );
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_creation_code_artifacts(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_object(obj) AND
    validate_json_object_keys(
            obj,
            array ['sourceMap', 'linkReferences'],
            array ['cborAuxdata']
    ) AND
    validate_code_artifacts_cbor_auxdata(coalesce(obj -> 'cborAuxdata', 'null'::jsonb)) AND
    validate_code_artifacts_link_references(obj -> 'linkReferences');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_runtime_code_artifacts(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_object(obj) AND
    validate_json_object_keys(
            obj,
            array ['sourceMap', 'linkReferences', 'immutableReferences'],
            array ['cborAuxdata']
    ) AND
    validate_code_artifacts_cbor_auxdata(coalesce(obj -> 'cborAuxdata', 'null'::jsonb)) AND
    validate_code_artifacts_link_references(obj -> 'linkReferences') AND
    validate_code_artifacts_immutable_references(obj -> 'immutableReferences');
END;
$$ LANGUAGE plpgsql;


ALTER TABLE compiled_contracts
    ADD CONSTRAINT compilation_artifacts_json_schema
        CHECK (validate_compilation_artifacts(compilation_artifacts));

ALTER TABLE compiled_contracts
    ADD CONSTRAINT creation_code_artifacts_json_schema
        CHECK (validate_creation_code_artifacts(creation_code_artifacts));

ALTER TABLE compiled_contracts
    ADD CONSTRAINT runtime_code_artifacts_json_schema
        CHECK (validate_runtime_code_artifacts(runtime_code_artifacts));

/*
    Validation functions to be used in `verified_contracts` values constraints.
*/
CREATE OR REPLACE FUNCTION validate_values_constructor_arguments(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
    -- `obj` does not contain 'constructorArguments' key
    IF NOT obj ? 'constructorArguments' THEN
        RETURN true;
END IF;

RETURN is_jsonb_string(obj -> 'constructorArguments')
    AND is_valid_hex(obj ->> 'constructorArguments', '+');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_values_libraries(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
    -- `obj` does not contain 'libraries' key
    IF NOT obj ? 'libraries' THEN
        RETURN true;
END IF;

    IF NOT is_jsonb_object(obj -> 'libraries') THEN
        RETURN false;
END IF;

RETURN bool_and(
        length(key) > 0 AND
        is_jsonb_string(value) AND
        is_valid_hex(value ->> 0, '{20}')
       )
    FROM jsonb_each(obj -> 'libraries');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_values_immutables(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
    -- `obj` does not contain 'immutables' key
    IF NOT obj ? 'immutables' THEN
        RETURN true;
END IF;

    IF NOT is_jsonb_object(obj -> 'immutables') THEN
        RETURN false;
END IF;

RETURN bool_and(
        length(key) > 0 AND
        is_jsonb_string(value) AND
        is_valid_hex(value ->> 0, '+')
       )
    FROM jsonb_each(obj -> 'immutables');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_values_cbor_auxdata(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
    -- `obj` does not contain 'cborAuxdata' key
    IF NOT obj ? 'cborAuxdata' THEN
        RETURN true;
END IF;

    IF NOT is_jsonb_object(obj -> 'cborAuxdata') THEN
        RETURN false;
END IF;

RETURN bool_and(
        length(key) > 0 AND
        is_jsonb_string(value) AND
        is_valid_hex(value ->> 0, '+')
       )
    FROM jsonb_each(obj -> 'cborAuxdata');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_values_call_protection(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
    -- `obj` does not contain 'callProtection' key
    IF NOT obj ? 'callProtection' THEN
        RETURN true;
END IF;

RETURN is_jsonb_string(obj -> 'callProtection')
    AND is_valid_hex(obj ->> 'callProtection', '{20}');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_creation_values(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_object(obj) AND
    validate_json_object_keys(
            obj,
            array []::text[],
            array ['constructorArguments', 'libraries', 'cborAuxdata']
    ) AND
    validate_values_constructor_arguments(obj) AND
    validate_values_libraries(obj) AND
    validate_values_cbor_auxdata(obj);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_runtime_values(obj jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_object(obj) AND
    validate_json_object_keys(
            obj,
            array []::text[],
            array ['libraries', 'immutables', 'cborAuxdata', 'callProtection']
    ) AND
    validate_values_libraries(obj) AND
    validate_values_immutables(obj) AND
    validate_values_cbor_auxdata(obj) AND
    validate_values_call_protection(obj);
END;
$$ LANGUAGE plpgsql;


CREATE OR REPLACE FUNCTION validate_transformation_key_type(object jsonb, expected_value text)
    RETURNS boolean AS
$$
BEGIN
RETURN object ? 'type' AND is_jsonb_string(object -> 'type') AND object ->> 'type' = expected_value;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformation_key_offset(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN object ? 'offset' AND is_jsonb_number(object -> 'offset') AND (object ->> 'offset')::integer >= 0;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformation_key_id(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN object ? 'id' AND is_jsonb_string(object -> 'id') AND length(object ->> 'id') > 0;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformations_constructor_arguments(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN validate_transformation_key_type(object, 'insert') AND validate_transformation_key_offset(object);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformations_library(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN validate_transformation_key_type(object, 'replace') AND validate_transformation_key_offset(object)
    AND validate_transformation_key_id(object);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformations_immutable(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN (validate_transformation_key_type(object, 'replace') OR validate_transformation_key_type(object, 'insert')) AND validate_transformation_key_offset(object)
    AND validate_transformation_key_id(object);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformations_cbor_auxdata(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN validate_transformation_key_type(object, 'replace') AND validate_transformation_key_offset(object)
    AND validate_transformation_key_id(object);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_transformations_call_protection(object jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN validate_transformation_key_type(object, 'replace')
    -- 'callProtection' value is always located at offset 1
    AND validate_transformation_key_offset(object) AND (object ->> 'offset')::integer = 1;
END;
$$ LANGUAGE plpgsql;


CREATE OR REPLACE FUNCTION validate_transformations(transformations jsonb, allowed_reasons text[])
    RETURNS boolean AS
$$
DECLARE
transformation_object jsonb;
    reason                text;
BEGIN
FOR transformation_object IN SELECT * FROM jsonb_array_elements(transformations)
                                               LOOP
    IF NOT is_jsonb_object(transformation_object)
                OR NOT transformation_object ? 'reason'
                OR NOT is_jsonb_string(transformation_object -> 'reason')
                OR array_position(allowed_reasons, transformation_object ->> 'reason') IS NULL
            THEN
                RETURN false;
END IF;

            reason := transformation_object ->> 'reason';

CASE
                WHEN reason = 'constructorArguments'
                    THEN RETURN validate_transformations_constructor_arguments(transformation_object);
WHEN reason = 'library' THEN RETURN validate_transformations_library(transformation_object);
WHEN reason = 'immutable' THEN RETURN validate_transformations_immutable(transformation_object);
WHEN reason = 'cborAuxdata' THEN RETURN validate_transformations_cbor_auxdata(transformation_object);
WHEN reason = 'callProtection'
                    THEN RETURN validate_transformations_call_protection(transformation_object);
ELSE
END CASE;

END LOOP;

RETURN true;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_creation_transformations(transformations jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_array(transformations) AND
    validate_transformations(transformations, array ['constructorArguments', 'library', 'cborAuxdata']);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_runtime_transformations(transformations jsonb)
    RETURNS boolean AS
$$
BEGIN
RETURN
    is_jsonb_array(transformations) AND
    validate_transformations(transformations, array ['library', 'immutable', 'cborAuxdata', 'callProtection']);
END;
$$ LANGUAGE plpgsql;

ALTER TABLE verified_contracts
    ADD CONSTRAINT creation_values_json_schema
        CHECK (creation_values IS NULL OR validate_creation_values(creation_values));

ALTER TABLE verified_contracts
    ADD CONSTRAINT runtime_values_json_schema
        CHECK (runtime_values IS NULL OR validate_runtime_values(runtime_values));

ALTER TABLE verified_contracts
    ADD CONSTRAINT creation_transformations_json_schema
        CHECK (creation_transformations IS NULL OR validate_creation_transformations(creation_transformations));

ALTER TABLE verified_contracts
    ADD CONSTRAINT runtime_transformations_json_schema
        CHECK (runtime_transformations IS NULL OR validate_runtime_transformations(runtime_transformations));

/*
    Set up timestamps related triggers. Used to enforce `created_at` and `updated_at`
    specific rules and prevent users to set those columns to invalid values.
    Spefically:
        `created_at` - should be set to the current timestamp on new row insertion,
                        and should not be modified after that.
        `updated_at` - should be set to the current timestamp on new row insertion,
                        and should be always be updated the corresponding value is modified.
*/

/* Needed to automatically set `created_at` fields on insertions. */
CREATE FUNCTION trigger_set_created_at()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.created_at = NOW();
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

/*  Needed to prevent modifying `created_at` fields on updates */
CREATE FUNCTION trigger_reuse_created_at()
    RETURNS TRIGGER AS
    $$
BEGIN
    NEW.created_at = OLD.created_at;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

/* Needed to automatically set `updated_at` fields on insertions and updates */
CREATE FUNCTION trigger_set_updated_at()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO
$$
    DECLARE
t_name text;
BEGIN
FOR t_name IN (VALUES ('code'),
                              ('contracts'),
                              ('contract_deployments'),
                              ('compiled_contracts'),
                              ('sources'),
                              ('verified_contracts'))
            LOOP
                EXECUTE format('CREATE TRIGGER insert_set_created_at
                        BEFORE INSERT ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_set_created_at()',
                               t_name);

EXECUTE format('CREATE TRIGGER insert_set_updated_at
                        BEFORE INSERT ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_set_updated_at()',
               t_name);

EXECUTE format('CREATE TRIGGER update_reuse_created_at
                        BEFORE UPDATE ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_reuse_created_at()',
               t_name);

EXECUTE format('CREATE TRIGGER update_set_updated_at
                        BEFORE UPDATE ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_set_updated_at()',
               t_name);
END LOOP;
END;
$$ LANGUAGE plpgsql;

/*
    Set up ownership (who inserted the value) related triggers.
    Used to enforce `created_by` and `updated_by` specific rules and prevent users to
    set those columns to invalid values.
    Spefically:
        `created_by` - should be set to the current user on new row insertion,
                        and should not be modified after that.
        `updated_by` - should be set to the current user on new row insertion,
                        and should be always be updated the corresponding value is modified.
*/

/* Needed to automatically set `created_by` fields on insertions. */
CREATE FUNCTION trigger_set_created_by()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.created_by = current_user;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

/*  Needed to prevent modifying `created_by` fields on updates */
CREATE FUNCTION trigger_reuse_created_by()
    RETURNS TRIGGER AS
    $$
BEGIN
    NEW.created_by = OLD.created_by;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

/* Needed to automatically set `updated_by` fields on insertions and updates */
CREATE FUNCTION trigger_set_updated_by()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_by = current_user;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;


/* Set up ownership related triggers */
DO
$$
    DECLARE
t_name text;
BEGIN
FOR t_name IN (VALUES ('code'),
                              ('contracts'),
                              ('contract_deployments'),
                              ('compiled_contracts'),
                              ('sources'),
                              ('verified_contracts'))
            LOOP
                EXECUTE format('CREATE TRIGGER insert_set_created_by
                        BEFORE INSERT ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_set_created_by()',
                               t_name);

EXECUTE format('CREATE TRIGGER insert_set_updated_by
                        BEFORE INSERT ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_set_updated_by()',
               t_name);

EXECUTE format('CREATE TRIGGER update_reuse_created_by
                        BEFORE UPDATE ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_reuse_created_by()',
               t_name);

EXECUTE format('CREATE TRIGGER update_set_updated_by
                        BEFORE UPDATE ON %I
                            FOR EACH ROW
                        EXECUTE FUNCTION trigger_set_updated_by()',
               t_name);
END LOOP;
END;
$$ LANGUAGE plpgsql;