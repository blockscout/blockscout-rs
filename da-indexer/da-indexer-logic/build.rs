fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile_protos(
        &["src/eigenda/proto/disperser/disperser.proto"],
        &["src/eigenda/proto"],
    )?;
    Ok(())
}
