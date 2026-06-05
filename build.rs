fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(
            &["proto/cli/runner/v1/runner.proto","proto/cli/user/v1/user.proto",],
            &["proto"],
        )?;
    Ok(())
}