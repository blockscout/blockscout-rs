fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/proto")
        .protoc_arg("--openapiv2_out=proto")
        .compile(
            &["proto/service.proto"],
            &["proto/", "../proto/googleapis", "../proto/grpc-gateway"],
        )?;
    Ok(())
}
