use crate::{
    compiler::{self, Compilers, Version},
    http_server::{handlers::verification::VerificationResponse, metrics},
    vyper::VyperCompilerAgent,
    VerificationResult,
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use ethers_solc::CompilerInput;
use std::{collections::BTreeMap, str::FromStr};
use tracing::instrument;

use super::types::VyperVerificationRequest;

#[instrument(skip(compilers, params), level = "debug")]
pub async fn verify(
    compilers: web::Data<Compilers<VyperCompilerAgent>>,
    params: Json<VyperVerificationRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    let request = params.into_inner();

    let compiler_input =
        CompilerInput::try_from(request.clone()).map_err(error::ErrorBadRequest)?;
    let compiler_version =
        Version::from_str(&request.compiler_version).map_err(error::ErrorBadRequest)?;

    let input = Input {
        compiler_version,
        compiler_input,
        creation_tx_input: &request.creation_bytecode,
        deployed_bytecode: &request.deployed_bytecode,
    };

    let response = compile_and_verify_handler(&compilers, input).await?;
    metrics::count_verify_contract(&response.status, "multi-part");
    Ok(Json(response))
}

#[derive(Debug)]
pub struct Input<'a> {
    pub compiler_version: compiler::Version,
    pub compiler_input: CompilerInput,
    pub creation_tx_input: &'a str,
    pub deployed_bytecode: &'a str,
}

async fn compile_and_verify_handler(
    compilers: &Compilers<VyperCompilerAgent>,
    input: Input<'_>,
) -> Result<VerificationResponse, actix_web::Error> {
    let result = compilers
        .compile(&input.compiler_version, &input.compiler_input)
        .await;
    let _output = match result {
        Ok(output) => output,
        Err(e) => return Ok(VerificationResponse::err(e)),
    };
    // let bytecodes: Vec<Option<String>> = output
    //     .contracts_iter()
    //     .map(|(_, c)| {
    //         c.get_bytecode_bytes()
    //             .and_then(|b| Some(hex::encode(b.to_vec())))
    //     })
    //     .collect();
    // println!("{:?}", bytecodes);

    // TODO: actual verification
    Ok(VerificationResponse::ok(VerificationResult {
        file_name: "".into(),
        contract_name: "".into(),
        compiler_version: "".into(),
        evm_version: "".into(),
        constructor_arguments: None,
        optimization: None,
        optimization_runs: None,
        contract_libraries: BTreeMap::new(),
        abi: "".into(),
        sources: BTreeMap::new(),
    }))
}
