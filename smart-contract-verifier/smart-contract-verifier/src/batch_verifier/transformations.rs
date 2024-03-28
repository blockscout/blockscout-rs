use super::artifacts::{compilation_artifacts::CompilationArtifacts, CodeArtifacts};
use crate::batch_verifier::errors::VerificationErrorKind;
use alloy_dyn_abi::JsonAbiExt;
use anyhow::Context;
use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum TransformationType {
    Insert,
    Replace,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum TransformationReason {
    Auxdata,
    Constructor,
    Immutable,
    Library,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Transformation {
    r#type: TransformationType,
    reason: TransformationReason,
    offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

impl Transformation {
    pub fn auxdata(offset: usize, id: String) -> Self {
        Self {
            r#type: TransformationType::Replace,
            reason: TransformationReason::Auxdata,
            offset,
            id: Some(id),
        }
    }

    pub fn constructor(offset: usize) -> Self {
        Self {
            r#type: TransformationType::Insert,
            reason: TransformationReason::Constructor,
            offset,
            id: None,
        }
    }

    pub fn immutable(offset: usize, id: String) -> Self {
        Self {
            r#type: TransformationType::Replace,
            reason: TransformationReason::Immutable,
            offset,
            id: Some(id),
        }
    }

    pub fn library(offset: usize, id: String) -> Self {
        Self {
            r#type: TransformationType::Replace,
            reason: TransformationReason::Library,
            offset,
            id: Some(id),
        }
    }
}

pub fn process_creation_code(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    compilation_artifacts: &CompilationArtifacts,
    code_artifacts: CodeArtifacts,
) -> Result<(Vec<u8>, serde_json::Value, serde_json::Value), VerificationErrorKind> {
    let processors = vec![
        process_libraries,
        process_cbor_auxdata,
        process_constructor_arguments,
    ];

    process_code(
        deployed_code,
        compiled_code,
        compilation_artifacts,
        code_artifacts,
        processors,
    )
}

pub fn process_runtime_code(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    compilation_artifacts: &CompilationArtifacts,
    code_artifacts: CodeArtifacts,
) -> Result<(Vec<u8>, serde_json::Value, serde_json::Value), VerificationErrorKind> {
    let processors = vec![process_immutables, process_libraries, process_cbor_auxdata];

    process_code(
        deployed_code,
        compiled_code,
        compilation_artifacts,
        code_artifacts,
        processors,
    )
}

fn process_code<F>(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    compilation_artifacts: &CompilationArtifacts,
    code_artifacts: CodeArtifacts,
    processors: Vec<F>,
) -> Result<(Vec<u8>, serde_json::Value, serde_json::Value), VerificationErrorKind>
where
    F: Fn(
        &[u8],
        Vec<u8>,
        &CompilationArtifacts,
        CodeArtifacts,
    ) -> Result<ProcessingResult, VerificationErrorKind>,
{
    // Just to ensure that further access by indexes will not certainly panic.
    // Not sure, if that condition may actually be true.
    if deployed_code.len() < compiled_code.len() {
        return Err(
            anyhow::anyhow!("deployed code length is less than the result of compilation").into(),
        );
    }

    let mut values: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut transformations: Vec<Transformation> = Vec::new();

    let mut transforming_code = compiled_code;
    for processor in processors {
        let result = processor(
            deployed_code,
            transforming_code,
            compilation_artifacts,
            code_artifacts.clone(),
        )
        .context("code processing failed")?;

        transforming_code = result.transformed_code;
        // will iterate through the Option and will not add anything if the value is `None`
        values.extend(result.values);
        transformations.extend(result.transformations);
    }

    Ok((
        transforming_code,
        serde_json::to_value(values).unwrap(),
        serde_json::to_value(transformations).unwrap(),
    ))
}

struct ProcessingResult {
    transformed_code: Vec<u8>,
    values: Option<(String, serde_json::Value)>,
    transformations: Vec<Transformation>,
}

fn process_cbor_auxdata(
    deployed_code: &[u8],
    mut compiled_code: Vec<u8>,
    _compilation_artifacts: &CompilationArtifacts,
    code_artifacts: CodeArtifacts,
) -> Result<ProcessingResult, VerificationErrorKind> {
    let mut auxdata_values: BTreeMap<String, DisplayBytes> = BTreeMap::new();
    let mut transformations = vec![];

    let cbor_auxdata = code_artifacts.cbor_auxdata();
    for (id, auxdata) in cbor_auxdata {
        let range = auxdata.offset..auxdata.offset + auxdata.value.len();
        let deployed_code_value = &deployed_code[range.clone()];

        if &compiled_code[range.clone()] != deployed_code_value {
            auxdata_values.insert(id.clone(), DisplayBytes::from(deployed_code_value.to_vec()));

            let transformation = Transformation::auxdata(auxdata.offset, id.clone());
            transformations.push(transformation);

            compiled_code[range].copy_from_slice(deployed_code_value);
        }
    }

    let values = (!auxdata_values.is_empty()).then_some((
        "cborAuxdata".to_string(),
        serde_json::to_value(auxdata_values).unwrap(),
    ));

    Ok(ProcessingResult {
        transformed_code: compiled_code,
        values,
        transformations,
    })
}

fn process_constructor_arguments(
    deployed_code: &[u8],
    mut compiled_code: Vec<u8>,
    compilation_artifacts: &CompilationArtifacts,
    _code_artifacts: CodeArtifacts,
) -> Result<ProcessingResult, VerificationErrorKind> {
    let offset = compiled_code.len();

    let (_, arguments) = deployed_code.split_at(offset);

    let constructor = match compilation_artifacts.abi.as_ref() {
        Some(abi) => {
            alloy_json_abi::JsonAbi::from_json_str(&abi.to_string())
                .context("parse json abi from compilation artifacts")?
                .constructor
        }
        None => None,
    };

    let invalid_constructor_arguments = || {
        Err(VerificationErrorKind::InvalidConstructorArguments(
            DisplayBytes::from(arguments.to_vec()),
        ))
    };
    let (values, transformations) = match constructor {
        None if !arguments.is_empty() => return invalid_constructor_arguments(),
        Some(_constructor) if arguments.is_empty() => return invalid_constructor_arguments(),
        None => (None, vec![]),
        Some(constructor) => {
            if let Err(_err) = constructor.abi_decode_input(arguments, true) {
                return invalid_constructor_arguments();
            }

            let transformations = vec![Transformation::constructor(offset)];
            let values = Some((
                "constructorArguments".to_string(),
                serde_json::to_value(DisplayBytes::from(arguments.to_vec())).unwrap(),
            ));

            compiled_code.extend(arguments);

            (values, transformations)
        }
    };

    Ok(ProcessingResult {
        transformed_code: compiled_code,
        values,
        transformations,
    })
}

fn process_libraries(
    deployed_code: &[u8],
    mut compiled_code: Vec<u8>,
    _compilation_artifacts: &CompilationArtifacts,
    code_artifacts: CodeArtifacts,
) -> Result<ProcessingResult, VerificationErrorKind> {
    let mut library_values: BTreeMap<String, DisplayBytes> = BTreeMap::new();
    let mut transformations = vec![];

    let link_references = code_artifacts
        .try_link_references()
        .context("get link references from code artifacts")?;
    for (file, libraries) in link_references {
        for (contract, references) in libraries {
            let id = format!("{file}:{contract}");

            for (index, reference) in references.iter().enumerate() {
                let start = reference.start as usize;
                let length = reference.length as usize;

                let range = start..start + length;
                let deployed_code_value = &deployed_code[range.clone()];

                // The deployed_code values are the same for all references for the given id.
                // So we need to insert that value only once.
                if index == 0 {
                    library_values
                        .insert(id.clone(), DisplayBytes::from(deployed_code_value.to_vec()));
                }

                let transformation = Transformation::library(start, id.clone());
                transformations.push(transformation);

                compiled_code[range].copy_from_slice(deployed_code_value);
            }
        }
    }

    let values = (!library_values.is_empty()).then_some((
        "libraries".to_string(),
        serde_json::to_value(library_values).unwrap(),
    ));

    Ok(ProcessingResult {
        transformed_code: compiled_code,
        values,
        transformations,
    })
}

fn process_immutables(
    deployed_code: &[u8],
    mut compiled_code: Vec<u8>,
    _compilation_artifacts: &CompilationArtifacts,
    code_artifacts: CodeArtifacts,
) -> Result<ProcessingResult, VerificationErrorKind> {
    let mut immutable_values: BTreeMap<String, DisplayBytes> = BTreeMap::new();
    let mut transformations = vec![];

    let immutable_references = code_artifacts
        .try_immutable_references()
        .context("get immutable references from code artifacts")?;
    for (id, references) in immutable_references {
        for (index, reference) in references.iter().enumerate() {
            let start = reference.start as usize;
            let length = reference.length as usize;

            let range = start..start + length;
            let deployed_code_value = &deployed_code[range.clone()];

            // The deployed_code values are the same for all references for the given id.
            // So we need to insert that value only once.
            if index == 0 {
                immutable_values
                    .insert(id.clone(), DisplayBytes::from(deployed_code_value.to_vec()));
            }

            let transformation = Transformation::immutable(start, id.clone());
            transformations.push(transformation);

            compiled_code[range].copy_from_slice(deployed_code_value);
        }
    }

    let values = (!immutable_values.is_empty()).then_some((
        "immutables".to_string(),
        serde_json::to_value(immutable_values).unwrap(),
    ));

    Ok(ProcessingResult {
        transformed_code: compiled_code,
        values,
        transformations,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        super::artifacts::{
            creation_code_artifacts::CreationCodeArtifacts,
            runtime_code_artifacts::RuntimeCodeArtifacts,
        },
        *,
    };
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    struct TestInput<'a, F> {
        deployed_code: &'a str,
        compiled_code: &'a str,
        code_artifacts: CodeArtifacts,
        processors: Vec<F>,
    }

    fn compilation_artifacts() -> CompilationArtifacts {
        CompilationArtifacts {
            abi: Some(serde_json::json!([
                {
                    "inputs": [
                        {
                            "internalType": "uint256",
                            "name": "_a",
                            "type": "uint256"
                        }
                    ],
                    "stateMutability": "nonpayable",
                    "type": "constructor"
                }
            ])),
            devdoc: None,
            userdoc: None,
            storage_layout: None,
            sources: Default::default(),
        }
    }

    fn test_processing_function<F>(
        input_data: TestInput<'_, F>,
        expected_values: serde_json::Value,
        expected_transformations: serde_json::Value,
    ) where
        F: Fn(
            &[u8],
            Vec<u8>,
            &CompilationArtifacts,
            CodeArtifacts,
        ) -> Result<ProcessingResult, VerificationErrorKind>,
    {
        let deployed_code = DisplayBytes::from_str(input_data.deployed_code)
            .expect("Invalid deployed code")
            .to_vec();
        let compiled_code = DisplayBytes::from_str(input_data.compiled_code)
            .expect("Invalid compiled code")
            .to_vec();

        let (transformed_code, values, transformations) = process_code(
            &deployed_code,
            compiled_code,
            &compilation_artifacts(),
            input_data.code_artifacts,
            input_data.processors,
        )
        .expect("processing failed");

        assert_eq!(deployed_code, transformed_code, "Invalid transformed code");
        assert_eq!(expected_values, values, "Invalid values");
        assert_eq!(
            expected_transformations, transformations,
            "Invalid transformations"
        );
    }

    #[test]
    fn test_process_cbor_auxdata() {
        let code_artifacts = serde_json::json!({
            "cborAuxdata": {
                "1": {
                    "offset": 1639,
                    "value": "0xa264697066735822122005d1b64ca59de3c6d96eee72b6fef65fc503bfbf8d9719fb047fafce2ebdc29764736f6c63430008120033"
                },
                "2": {
                    "offset": 1731,
                    "value": "0xa2646970667358221220aebf48746b808da25305449bba6945baacf1c2185dfcc58a94b1506b8b5a6dfa64736f6c63430008120033"
                }
            }
        });
        let code_artifacts: CreationCodeArtifacts =
            serde_json::from_value(code_artifacts.clone()).unwrap();

        let deployed_code = "0x60806040526040518060200161001490610049565b6020820181038252601f19601f820116604052506001908161003691906102a5565b5034801561004357600080fd5b50610377565b605c8061069c83390190565b600081519050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806100d657607f821691505b6020821081036100e9576100e861008f565b5b50919050565b60008190508160005260206000209050919050565b60006020601f8301049050919050565b600082821b905092915050565b6000600883026101517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82610114565b61015b8683610114565b95508019841693508086168417925050509392505050565b6000819050919050565b6000819050919050565b60006101a261019d61019884610173565b61017d565b610173565b9050919050565b6000819050919050565b6101bc83610187565b6101d06101c8826101a9565b848454610121565b825550505050565b600090565b6101e56101d8565b6101f08184846101b3565b505050565b5b81811015610214576102096000826101dd565b6001810190506101f6565b5050565b601f8211156102595761022a816100ef565b61023384610104565b81016020851015610242578190505b61025661024e85610104565b8301826101f5565b50505b505050565b600082821c905092915050565b600061027c6000198460080261025e565b1980831691505092915050565b6000610295838361026b565b9150826002028217905092915050565b6102ae82610055565b67ffffffffffffffff8111156102c7576102c6610060565b5b6102d182546100be565b6102dc828285610218565b600060209050601f83116001811461030f57600084156102fd578287015190505b6103078582610289565b86555061036f565b601f19841661031d866100ef565b60005b8281101561034557848901518255600182019150602085019450602081019050610320565b86831015610362578489015161035e601f89168261026b565b8355505b6001600288020188555050505b505050505050565b610316806103866000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c806324c12bf6146100465780636057361d146100645780638381f58a14610080575b600080fd5b61004e61009e565b60405161005b91906101cc565b60405180910390f35b61007e60048036038101906100799190610229565b61012c565b005b610088610136565b6040516100959190610265565b60405180910390f35b600180546100ab906102af565b80601f01602080910402602001604051908101604052809291908181526020018280546100d7906102af565b80156101245780601f106100f957610100808354040283529160200191610124565b820191906000526020600020905b81548152906001019060200180831161010757829003601f168201915b505050505081565b8060008190555050565b60005481565b600081519050919050565b600082825260208201905092915050565b60005b8381101561017657808201518184015260208101905061015b565b60008484015250505050565b6000601f19601f8301169050919050565b600061019e8261013c565b6101a88185610147565b93506101b8818560208601610158565b6101c181610182565b840191505092915050565b600060208201905081810360008301526101e68184610193565b905092915050565b600080fd5b6000819050919050565b610206816101f3565b811461021157600080fd5b50565b600081359050610223816101fd565b92915050565b60006020828403121561023f5761023e6101ee565b5b600061024d84828501610214565b91505092915050565b61025f816101f3565b82525050565b600060208201905061027a6000830184610256565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806102c757607f821691505b6020821081036102da576102d9610280565b5b5091905056fea2646970667358221220bc2c6d72c52842d4077bb24c307576e44a078831aaa16da6611ef342fd052ec764736f6c634300081200336080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea2646970667358221220f13d144a826a3f18798a534a4b10029a3284d9f4620ccc79750cdc48442cdaad64736f6c63430008120033";
        let compiled_code = "0x60806040526040518060200161001490610049565b6020820181038252601f19601f820116604052506001908161003691906102a5565b5034801561004357600080fd5b50610377565b605c8061069c83390190565b600081519050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806100d657607f821691505b6020821081036100e9576100e861008f565b5b50919050565b60008190508160005260206000209050919050565b60006020601f8301049050919050565b600082821b905092915050565b6000600883026101517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82610114565b61015b8683610114565b95508019841693508086168417925050509392505050565b6000819050919050565b6000819050919050565b60006101a261019d61019884610173565b61017d565b610173565b9050919050565b6000819050919050565b6101bc83610187565b6101d06101c8826101a9565b848454610121565b825550505050565b600090565b6101e56101d8565b6101f08184846101b3565b505050565b5b81811015610214576102096000826101dd565b6001810190506101f6565b5050565b601f8211156102595761022a816100ef565b61023384610104565b81016020851015610242578190505b61025661024e85610104565b8301826101f5565b50505b505050565b600082821c905092915050565b600061027c6000198460080261025e565b1980831691505092915050565b6000610295838361026b565b9150826002028217905092915050565b6102ae82610055565b67ffffffffffffffff8111156102c7576102c6610060565b5b6102d182546100be565b6102dc828285610218565b600060209050601f83116001811461030f57600084156102fd578287015190505b6103078582610289565b86555061036f565b601f19841661031d866100ef565b60005b8281101561034557848901518255600182019150602085019450602081019050610320565b86831015610362578489015161035e601f89168261026b565b8355505b6001600288020188555050505b505050505050565b610316806103866000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c806324c12bf6146100465780636057361d146100645780638381f58a14610080575b600080fd5b61004e61009e565b60405161005b91906101cc565b60405180910390f35b61007e60048036038101906100799190610229565b61012c565b005b610088610136565b6040516100959190610265565b60405180910390f35b600180546100ab906102af565b80601f01602080910402602001604051908101604052809291908181526020018280546100d7906102af565b80156101245780601f106100f957610100808354040283529160200191610124565b820191906000526020600020905b81548152906001019060200180831161010757829003601f168201915b505050505081565b8060008190555050565b60005481565b600081519050919050565b600082825260208201905092915050565b60005b8381101561017657808201518184015260208101905061015b565b60008484015250505050565b6000601f19601f8301169050919050565b600061019e8261013c565b6101a88185610147565b93506101b8818560208601610158565b6101c181610182565b840191505092915050565b600060208201905081810360008301526101e68184610193565b905092915050565b600080fd5b6000819050919050565b610206816101f3565b811461021157600080fd5b50565b600081359050610223816101fd565b92915050565b60006020828403121561023f5761023e6101ee565b5b600061024d84828501610214565b91505092915050565b61025f816101f3565b82525050565b600060208201905061027a6000830184610256565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806102c757607f821691505b6020821081036102da576102d9610280565b5b5091905056fea264697066735822122005d1b64ca59de3c6d96eee72b6fef65fc503bfbf8d9719fb047fafce2ebdc29764736f6c634300081200336080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea2646970667358221220aebf48746b808da25305449bba6945baacf1c2185dfcc58a94b1506b8b5a6dfa64736f6c63430008120033";

        let processors = vec![process_cbor_auxdata];

        let expected_values = serde_json::json!({
            "cborAuxdata": {
                "1": "0xa2646970667358221220bc2c6d72c52842d4077bb24c307576e44a078831aaa16da6611ef342fd052ec764736f6c63430008120033",
                "2": "0xa2646970667358221220f13d144a826a3f18798a534a4b10029a3284d9f4620ccc79750cdc48442cdaad64736f6c63430008120033"
            }
        });

        let expected_transformations = serde_json::json!([
            {
                "type": "replace",
                "reason": "auxdata",
                "offset": 1639,
                "id": "1"
            },
            {
                "type": "replace",
                "reason": "auxdata",
                "offset": 1731,
                "id": "2"
            },
        ]);

        test_processing_function(
            TestInput {
                deployed_code,
                compiled_code,
                code_artifacts: CodeArtifacts::CreationCodeArtifacts(code_artifacts),
                processors: processors.clone(),
            },
            expected_values.clone(),
            expected_transformations.clone(),
        );
    }

    #[test]
    fn test_process_constructor_arguments() {
        let deployed_code = "0x608060405234801561001057600080fd5b506040516101e93803806101e98339818101604052810190610032919061007a565b80600081905550506100a7565b600080fd5b6000819050919050565b61005781610044565b811461006257600080fd5b50565b6000815190506100748161004e565b92915050565b6000602082840312156100905761008f61003f565b5b600061009e84828501610065565b91505092915050565b610133806100b66000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea2646970667358221220dd712ec4cb31d63cd32d3152e52e890b087769e9e4d6746844608039b5015d6a64736f6c634300081200330000000000000000000000000000000000000000000000000000000000003039";
        let compiled_code = "0x608060405234801561001057600080fd5b506040516101e93803806101e98339818101604052810190610032919061007a565b80600081905550506100a7565b600080fd5b6000819050919050565b61005781610044565b811461006257600080fd5b50565b6000815190506100748161004e565b92915050565b6000602082840312156100905761008f61003f565b5b600061009e84828501610065565b91505092915050565b610133806100b66000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea2646970667358221220dd712ec4cb31d63cd32d3152e52e890b087769e9e4d6746844608039b5015d6a64736f6c63430008120033";

        let processors = vec![process_constructor_arguments];

        let expected_values = serde_json::json!({
            "constructorArguments": "0x0000000000000000000000000000000000000000000000000000000000003039"
        });

        let expected_transformations = serde_json::json!([{
            "type": "insert",
            "reason": "constructor",
            "offset": 489
        }]);

        test_processing_function(
            TestInput {
                deployed_code,
                compiled_code,
                code_artifacts: CodeArtifacts::CreationCodeArtifacts(Default::default()),
                processors,
            },
            expected_values,
            expected_transformations,
        );
    }

    #[test]
    fn test_process_immutables() {
        let code_artifacts = serde_json::json!({
            "immutableReferences": {"7":[{"length":32,"start":176}]}
        });
        let code_artifacts: RuntimeCodeArtifacts =
            serde_json::from_value(code_artifacts.clone()).unwrap();

        let deployed_code = "0x608060405234801561001057600080fd5b50600436106100415760003560e01c80636057361d146100465780638381f58a146100625780639fe44c4a14610080575b600080fd5b610060600480360381019061005b919061010d565b61009e565b005b61006a6100a8565b6040516100779190610149565b60405180910390f35b6100886100ae565b6040516100959190610149565b60405180910390f35b8060008190555050565b60005481565b7f000000000000000000000000000000000000000000000000000000000000006481565b600080fd5b6000819050919050565b6100ea816100d7565b81146100f557600080fd5b50565b600081359050610107816100e1565b92915050565b600060208284031215610123576101226100d2565b5b6000610131848285016100f8565b91505092915050565b610143816100d7565b82525050565b600060208201905061015e600083018461013a565b9291505056fea26469706673582212205fff17b2676425e48225435ac15579ccae1af038ff8ffb334fc372526b94722664736f6c63430008120033";
        let compiled_code = "0x608060405234801561001057600080fd5b50600436106100415760003560e01c80636057361d146100465780638381f58a146100625780639fe44c4a14610080575b600080fd5b610060600480360381019061005b919061010d565b61009e565b005b61006a6100a8565b6040516100779190610149565b60405180910390f35b6100886100ae565b6040516100959190610149565b60405180910390f35b8060008190555050565b60005481565b7f000000000000000000000000000000000000000000000000000000000000000081565b600080fd5b6000819050919050565b6100ea816100d7565b81146100f557600080fd5b50565b600081359050610107816100e1565b92915050565b600060208284031215610123576101226100d2565b5b6000610131848285016100f8565b91505092915050565b610143816100d7565b82525050565b600060208201905061015e600083018461013a565b9291505056fea26469706673582212205fff17b2676425e48225435ac15579ccae1af038ff8ffb334fc372526b94722664736f6c63430008120033";

        let processors = vec![process_immutables];

        let expected_values = serde_json::json!({
            "immutables": {
                "7": "0x0000000000000000000000000000000000000000000000000000000000000064"
            }
        });

        let expected_transformations = serde_json::json!([{
            "type": "replace",
            "reason": "immutable",
            "offset": 176,
            "id": "7"
        }]);

        test_processing_function(
            TestInput {
                deployed_code,
                compiled_code,
                code_artifacts: CodeArtifacts::RuntimeCodeArtifacts(code_artifacts),
                processors: processors.clone(),
            },
            expected_values.clone(),
            expected_transformations.clone(),
        );
    }

    #[test]
    fn test_process_libraries() {
        let code_artifacts = serde_json::json!({
            "linkReferences": {"contracts/1_Storage.sol":{"Journal":[{"length":20,"start":217}]}}
        });
        let code_artifacts: CreationCodeArtifacts =
            serde_json::from_value(code_artifacts.clone()).unwrap();

        let deployed_code = "0x608060405234801561001057600080fd5b50610249806100206000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c80636057361d146100465780638381f58a14610062578063e2e2a85a14610080575b600080fd5b610060600480360381019061005b919061017d565b6100b0565b005b61006a610124565b60405161007791906101b9565b60405180910390f35b61009a6004803603810190610095919061017d565b61012a565b6040516100a791906101b9565b60405180910390f35b80600081905550737d53f102f4d4aa014db4e10d6deec2009b3cda6b632be59dd56001836040518363ffffffff1660e01b81526004016100f19291906101ea565b60006040518083038186803b15801561010957600080fd5b505af415801561011d573d6000803e3d6000fd5b5050505050565b60005481565b60016020528060005260406000206000915090505481565b600080fd5b6000819050919050565b61015a81610147565b811461016557600080fd5b50565b60008135905061017781610151565b92915050565b60006020828403121561019357610192610142565b5b60006101a184828501610168565b91505092915050565b6101b381610147565b82525050565b60006020820190506101ce60008301846101aa565b92915050565b8082525050565b6101e481610147565b82525050565b60006040820190506101ff60008301856101d4565b61020c60208301846101db565b939250505056fea26469706673582212205d40d1517b560d915e1b7c005887b93fcce9ec65c0a38a80ee147739551bdd7264736f6c63430008120033";
        let compiled_code = "0x608060405234801561001057600080fd5b50610249806100206000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c80636057361d146100465780638381f58a14610062578063e2e2a85a14610080575b600080fd5b610060600480360381019061005b919061017d565b6100b0565b005b61006a610124565b60405161007791906101b9565b60405180910390f35b61009a6004803603810190610095919061017d565b61012a565b6040516100a791906101b9565b60405180910390f35b80600081905550730000000000000000000000000000000000000000632be59dd56001836040518363ffffffff1660e01b81526004016100f19291906101ea565b60006040518083038186803b15801561010957600080fd5b505af415801561011d573d6000803e3d6000fd5b5050505050565b60005481565b60016020528060005260406000206000915090505481565b600080fd5b6000819050919050565b61015a81610147565b811461016557600080fd5b50565b60008135905061017781610151565b92915050565b60006020828403121561019357610192610142565b5b60006101a184828501610168565b91505092915050565b6101b381610147565b82525050565b60006020820190506101ce60008301846101aa565b92915050565b8082525050565b6101e481610147565b82525050565b60006040820190506101ff60008301856101d4565b61020c60208301846101db565b939250505056fea26469706673582212205d40d1517b560d915e1b7c005887b93fcce9ec65c0a38a80ee147739551bdd7264736f6c63430008120033";

        let processors = vec![process_libraries];

        let expected_values = serde_json::json!({
            "libraries": {
                "contracts/1_Storage.sol:Journal": "0x7d53f102f4d4aa014db4e10d6deec2009b3cda6b"
            }
        });

        let expected_transformations = serde_json::json!([{
            "type": "replace",
            "reason": "library",
            "offset": 217,
            "id": "contracts/1_Storage.sol:Journal"
        }]);

        test_processing_function(
            TestInput {
                deployed_code,
                compiled_code,
                code_artifacts: CodeArtifacts::CreationCodeArtifacts(code_artifacts),
                processors: processors.clone(),
            },
            expected_values.clone(),
            expected_transformations.clone(),
        );
    }
}
