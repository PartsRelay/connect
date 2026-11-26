# Parts Relay connect

The connectors between Parts Relay and the management systems of the sellers, in Rust.

- `grpc`: the gRPC link with the Parts Relay core, generated from its proto files.

Connect talks to the management systems over HTTPS and to the core only over gRPC. The core talks to the UI over HTTPS and never to a management system.

## grpc

The core and connect sit in the same private network and talk standard gRPC over cleartext HTTP/2. No key is exchanged: the network is what protects the calls, so neither gRPC port may be public. The link works both ways.

**The core calls connect.** `serve` answers `ArticleService` and the standard gRPC health check. A management system is a `Connector`, and the company a call is for comes in the metadata `workspace-id`:

```rust
use partsrelay_grpc::{Connector, serve, v1};

struct Shelf;

#[tonic::async_trait]
impl Connector for Shelf {
    async fn list_articles(
        &self,
        workspace_id: i64,
        request: v1::ListArticlesRequest,
    ) -> Result<v1::ListArticlesResponse, tonic::Status> {
        // Ask the management system of the company here
        Ok(v1::ListArticlesResponse::default())
    }
}

let listener = tokio::net::TcpListener::bind("0.0.0.0:50051").await?;
serve(listener, Shelf, std::future::pending()).await?;
```

**Connect calls the core.** `Core` is the client of the gRPC port of the core, today for its health:

```rust
let serving = partsrelay_grpc::Core::new("http://core:50051")?.is_serving().await?;
```

The proto files are a copy from [PartsRelay/protobuf](https://github.com/PartsRelay/protobuf), see `grpc/proto/README.md`.

## Checks

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```
