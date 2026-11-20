// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::proto;
use amplify::{From, Wrapper};
use eth_bytecode_db::search;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct FunctionDescriptionWrapper(proto::FunctionDescription);

impl From<search::FunctionDescription> for FunctionDescriptionWrapper {
    fn from(value: search::FunctionDescription) -> Self {
        FunctionDescriptionWrapper(proto::FunctionDescription {
            r#type: "function".to_string(),
            name: value.name,
            inputs: value.inputs.to_string(),
            signature: value.signature,
        })
    }
}
