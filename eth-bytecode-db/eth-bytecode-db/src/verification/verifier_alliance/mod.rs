mod parsers;
mod transformations;

pub fn process_creation_code(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    code_artifacts: serde_json::Value,
) -> Result<(serde_json::Value, serde_json::Value), anyhow::Error> {
    process_code(
        deployed_code,
        compiled_code,
        code_artifacts,
        transformations::process_creation_code,
    )
}

pub fn process_runtime_code(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    code_artifacts: serde_json::Value,
) -> Result<(serde_json::Value, serde_json::Value), anyhow::Error> {
    process_code(
        deployed_code,
        compiled_code,
        code_artifacts,
        transformations::process_runtime_code,
    )
}

fn process_code<F>(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    code_artifacts: serde_json::Value,
    processing_function: F,
) -> Result<(serde_json::Value, serde_json::Value), anyhow::Error>
where
    F: Fn(
        &[u8],
        Vec<u8>,
        serde_json::Value,
    ) -> Result<(Vec<u8>, serde_json::Value, serde_json::Value), anyhow::Error>,
{
    let (processed_code, values, transformations) =
        processing_function(deployed_code, compiled_code.clone(), code_artifacts)?;

    if processed_code != deployed_code {
        return Err(anyhow::anyhow!(
            "processed code does not match to the actually deployed one"
        ));
    }

    Ok((values, transformations))
}
