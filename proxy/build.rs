fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../proto/proxy.proto")?;
    tonic_build::compile_protos("../proto/client.proto")?;
    tonic_build::compile_protos("../proto/info.proto")?;
    tonic_build::compile_protos("../proto/auth.proto")?;
    Ok(())
}
