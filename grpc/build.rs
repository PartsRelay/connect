// Generates the server and the client from the proto files of PartsRelay/protobuf
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: a build script runs on one thread
    unsafe { std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?) };

    tonic_prost_build::compile_protos("proto/partsrelay/connect_api/v1/articles.proto")?;

    Ok(())
}
