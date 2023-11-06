use super::{
    disassemble::{disassemble_bytecode, DisassembledOpcode},
    method::Method,
};
use bytes::Bytes;
use ethers_core::abi::Abi;
use ethers_solc::sourcemap::SourceMap;
use std::{collections::BTreeMap, iter::repeat};

pub struct LookupMethodsRequest {
    pub bytecode: Bytes,
    pub abi: Abi,
    pub source_map: SourceMap,
    pub file_id_map: BTreeMap<u32, String>,
}

pub struct LookupMethodsResponse {
    pub methods: BTreeMap<String, Method>,
}

pub fn find_methods(request: LookupMethodsRequest) -> LookupMethodsResponse {
    let methods = parse_selectors(request.abi);
    let opcodes = disassemble_bytecode(&request.bytecode);
    let opcodes = opcodes.as_slice();

    let methods = methods
        .into_iter()
        .filter_map(|(func_sig, selector)| {
            let func_index = find_selector(&selector, opcodes).or_else(|| {
                tracing::warn!(
                    "function {} with selector '{}' not found in bytecode",
                    func_sig,
                    hex::encode(selector)
                );
                None
            })?;

            tracing::info!("found function {} in {}", func_sig, func_index);
            let method = Method::from_source_map(
                selector,
                &request.source_map,
                func_index,
                &request.file_id_map,
            )
            .unwrap();
            Some((hex::encode(selector), method))
        })
        .collect::<BTreeMap<String, Method>>();
    LookupMethodsResponse { methods }
}

fn prepend_selector(partial_selector: &Vec<u8>) -> Option<Vec<u8>> {
    if partial_selector.len() > 4 {
        return None;
    };

    // prepend selector with 0s if it's shorter than 4 bytes
    let mut selector = partial_selector.clone();
    selector.splice(..0, repeat(0).take(4 - partial_selector.len()));
    Some(selector)
}

fn find_selector(selector: &[u8; 4], opcodes: &[DisassembledOpcode]) -> Option<usize> {
    for window in opcodes.windows(5) {
        if window[0].operation.name.starts_with("PUSH")
            && window[1].operation.name == "EQ"
            && window[2].operation.name.starts_with("PUSH")
            && window[3].operation.name == "JUMPI"
        {
            let push_selector = prepend_selector(&window[0].args).expect("valid selector");

            if push_selector != selector {
                continue;
            }

            let jump_to =
                usize::from_str_radix(&hex::encode(&window[2].args), 16).expect("valid hex string");

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
            let push_selector = prepend_selector(&window[0].args).expect("valid selector");

            if push_selector != selector {
                continue;
            }

            let jump_to =
                usize::from_str_radix(&hex::encode(&window[3].args), 16).expect("valid hex string");

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

fn parse_selectors(abi: Abi) -> BTreeMap<String, [u8; 4]> {
    abi.functions()
        .map(|f| (f.signature(), f.short_signature()))
        .collect()
}
