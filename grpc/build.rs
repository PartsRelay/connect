// Generates the client from the proto files of PartsRelay/protobuf
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: a build script runs on one thread
    unsafe { std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?) };

    tonic_prost_build::configure()
        .build_server(false)
        // gRPC-Web over hyper, not the HTTP/2 channel of tonic
        .build_transport(false)
        .compile_protos(
            &["proto/partsrelay/public_api/v1/articles.proto"],
            &["proto"],
        )?;

    Ok(())
}
