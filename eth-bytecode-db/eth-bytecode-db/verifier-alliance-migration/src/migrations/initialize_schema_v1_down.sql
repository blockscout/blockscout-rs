DROP TABLE verified_contracts;
DROP TABLE compiled_contracts_sources;
DROP TABLE sources;
DROP TABLE compiled_contracts;
DROP TABLE contract_deployments;
DROP TABLE contracts;
DROP TABLE code;

DROP FUNCTION trigger_set_updated_by;
DROP FUNCTION trigger_reuse_created_by;
DROP FUNCTION trigger_set_created_by;

DROP FUNCTION trigger_set_updated_at;
DROP FUNCTION trigger_reuse_created_at;
DROP FUNCTION trigger_set_created_at;

DROP FUNCTION validate_runtime_transformations;
DROP FUNCTION validate_creation_transformations;
DROP FUNCTION validate_transformations;
DROP FUNCTION validate_transformations_call_protection;
DROP FUNCTION validate_transformations_cbor_auxdata;
DROP FUNCTION validate_transformations_immutable;
DROP FUNCTION validate_transformations_library;
DROP FUNCTION validate_transformations_constructor_arguments;
DROP FUNCTION validate_transformation_key_id;
DROP FUNCTION validate_transformation_key_offset;
DROP FUNCTION validate_transformation_key_type;

DROP FUNCTION validate_runtime_values;
DROP FUNCTION validate_creation_values;
DROP FUNCTION validate_values_call_protection;
DROP FUNCTION validate_values_cbor_auxdata;
DROP FUNCTION validate_values_immutables;
DROP FUNCTION validate_values_libraries;
DROP FUNCTION validate_values_constructor_arguments;

DROP FUNCTION validate_runtime_code_artifacts;
DROP FUNCTION validate_creation_code_artifacts;
DROP FUNCTION validate_compilation_artifacts;

DROP FUNCTION validate_json_object_keys;
DROP FUNCTION is_valid_hex;
DROP FUNCTION is_jsonb_number;
DROP FUNCTION is_jsonb_array;
DROP FUNCTION is_jsonb_string;
DROP FUNCTION is_jsonb_object;

DROP EXTENSION pgcrypto;