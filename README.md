# Parts Relay connect

The connectors between Parts Relay and the management systems of the sellers, in Rust.

- `grpc`: the client of the Parts Relay public API, generated from its proto files.

## grpc

The core answers its public API on its HTTPS port as gRPC-Web, so the client needs no HTTP/2 and passes any proxy. A call carries the API key of a company, a `test_` or a `live_` one.

```rust
use partsrelay_grpc::{Core, v1};

let mut core = Core::new("https://core.example", "test_...")?;
let page = core.list_articles(v1::ListArticlesRequest::default()).await?;
```

The proto files are a copy from [PartsRelay/protobuf](https://github.com/PartsRelay/protobuf), see `grpc/proto/README.md`.

## Checks

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```
