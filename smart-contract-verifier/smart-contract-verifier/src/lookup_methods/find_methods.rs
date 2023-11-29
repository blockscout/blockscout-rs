use super::{
    disassemble::{disassemble_bytecode, DisassembledOpcode},
    method::Method,
};
use crate::SoliditySuccess;
use bytes::Bytes;
use ethers_core::abi::Abi;
use ethers_solc::sourcemap::SourceMap;
use std::{collections::BTreeMap, iter::repeat};

pub struct LookupMethodsRequest {
    pub bytecode: Bytes,
    pub abi: Abi,
    pub source_map: SourceMap,
    pub file_ids: BTreeMap<u32, String>,
}

pub struct LookupMethodsResponse {
    pub methods: BTreeMap<String, Method>,
}

pub fn find_methods_from_compiler_output(
    res: &SoliditySuccess,
) -> anyhow::Result<LookupMethodsResponse> {
    let file_ids = res
        .compiler_output
        .sources
        .iter()
        .map(|(name, file)| (file.id, name.clone()))
        .collect();

    let path = &res.file_path;
    let file = res
        .compiler_output
        .contracts
        .get(path)
        .ok_or_else(|| anyhow::anyhow!("file {path} not found"))?;
    let contract_name = &res.contract_name;
    let contract = file
        .get(&res.contract_name)
        .ok_or_else(|| anyhow::anyhow!("contract {contract_name} not found"))?;

    let abi = &contract
        .abi
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("abi missing"))?
        .abi;

    let evm = contract
        .evm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("evm missing"))?;

    let deployed_bytecode = evm
        .deployed_bytecode
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("deployed bytecode missing"))?;
    let bytecode = deployed_bytecode
        .bytecode
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("bytecode missing"))?;

    let source_map = bytecode
        .source_map()
        .ok_or_else(|| anyhow::anyhow!("source map missing"))??;
    let bytecode_raw = &bytecode
        .object
        .as_bytes()
        .ok_or_else(|| anyhow::anyhow!("invalid bytecode"))?
        .0;

    let methods = parse_selectors(abi);

    Ok(find_methods_internal(
        methods,
        bytecode_raw,
        &source_map,
        &file_ids,
    ))
}

pub fn find_methods(request: LookupMethodsRequest) -> LookupMethodsResponse {
    let methods = parse_selectors(&request.abi);
    find_methods_internal(
        methods,
        &request.bytecode,
        &request.source_map,
        &request.file_ids,
    )
}

fn find_methods_internal(
    methods: BTreeMap<String, [u8; 4]>,
    bytecode: &Bytes,
    source_map: &SourceMap,
    file_ids: &BTreeMap<u32, String>,
) -> LookupMethodsResponse {
    let opcodes = disassemble_bytecode(bytecode);

    let methods = methods
        .into_iter()
        .filter_map(|(func_sig, selector)| {
            let func_index = match find_src_map_index(&selector, &opcodes) {
                Some(i) => i,
                None => {
                    tracing::warn!(func_sig, "function not found");
                    return None;
                }
            };

            let method = match Method::from_source_map(selector, source_map, func_index, file_ids) {
                Ok(m) => m,
                Err(err) => {
                    tracing::warn!(func_sig, err = err.to_string(), "failed to parse method");
                    return None;
                }
            };
            Some((hex::encode(selector), method))
        })
        .collect();
    LookupMethodsResponse { methods }
}

fn find_src_map_index(selector: &[u8; 4], opcodes: &[DisassembledOpcode]) -> Option<usize> {
    for window in opcodes.windows(5) {
        if window[0].operation.name.starts_with("PUSH")
            && window[1].operation.name == "EQ"
            && window[2].operation.name.starts_with("PUSH")
            && window[3].operation.name == "JUMPI"
        {
            // If found selector doesn't match, continue
            if !prepend_selector(&window[0].args).is_ok_and(|s| s == selector) {
                continue;
            }

            let jump_to = usize::from_str_radix(&hex::encode(&window[2].args), 16).ok()?;

            let maybe_target_opcode_index = opcodes
                .iter()
                .enumerate()
                .find_map(|(index, opcode)| (opcode.program_counter == jump_to).then_some(index));

            match maybe_target_opcode_index {
                Some(index) => return Some(index),
                None => tracing::warn!(selector =? selector, "target opcode not found"),
            }
        }

        if window[0].operation.name.starts_with("PUSH")
            && window[1].operation.name == "DUP2"
            && window[2].operation.name == "EQ"
            && window[3].operation.name.starts_with("PUSH")
            && window[4].operation.name == "JUMPI"
        {
            // If found selector doesn't match, continue
            if !prepend_selector(&window[0].args).is_ok_and(|s| s == selector) {
                continue;
            }

            let jump_to = usize::from_str_radix(&hex::encode(&window[3].args), 16).ok()?;

            let maybe_target_opcode_index = opcodes
                .iter()
                .enumerate()
                .find_map(|(index, opcode)| (opcode.program_counter == jump_to).then_some(index));

            match maybe_target_opcode_index {
                Some(index) => return Some(index),
                None => tracing::warn!(selector =? selector, "target opcode not found"),
            }
        }
    }

    None
}

fn parse_selectors(abi: &Abi) -> BTreeMap<String, [u8; 4]> {
    abi.functions()
        .map(|f| (f.signature(), f.short_signature()))
        .collect()
}

fn prepend_selector(partial_selector: &Vec<u8>) -> anyhow::Result<Vec<u8>> {
    if partial_selector.len() > 4 {
        return Err(anyhow::anyhow!("selector is too long"));
    };

    // prepend selector with 0s if it's shorter than 4 bytes
    let mut selector = partial_selector.clone();
    selector.splice(..0, repeat(0).take(4 - partial_selector.len()));
    Ok(selector)
}

#[cfg(test)]
mod tests {
    use super::prepend_selector;

    #[test]
    fn test_prepend_selector() {
        assert_eq!(
            prepend_selector(&vec![1, 2, 3, 4]).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(prepend_selector(&vec![1, 2]).unwrap(), vec![0, 0, 1, 2]);
        assert!(prepend_selector(&vec![1, 2, 3, 4, 5]).is_err());
    }
}
