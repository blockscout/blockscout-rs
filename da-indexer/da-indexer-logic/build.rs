fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile(
        &[
            "src/eigenda/proto/disperser/disperser.proto",
            "src/eigenda/proto/common/common.proto",
        ],
        &["src/eigenda/proto"],
    )?;
    Ok(())
}
