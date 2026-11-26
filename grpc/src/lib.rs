//! The gRPC link between connect and the core, generated from the proto files.
//!
//! The two services sit in the same private network and talk standard gRPC
//! over cleartext HTTP/2. No key is exchanged: the network protects the calls.

use std::future::Future;

use tokio::net::TcpListener;
use tonic::metadata::MetadataMap;
use tonic::transport::server::TcpIncoming;
use tonic::transport::{Channel, Endpoint, Error, Server};
use tonic::{Request, Response, Status};
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;

/// The messages, the server and the client of `partsrelay.public_api.v1`.
pub mod v1 {
    tonic::include_proto!("partsrelay.public_api.v1");
}

use v1::article_service_server::{ArticleService, ArticleServiceServer};

/// The metadata that tells which company a call is for.
pub const WORKSPACE_METADATA: &str = "workspace-id";

/// A management system, as the core asks about it.
#[tonic::async_trait]
pub trait Connector: Send + Sync + 'static {
    /// The articles of the company `workspace_id`.
    async fn list_articles(
        &self,
        workspace_id: i64,
        request: v1::ListArticlesRequest,
    ) -> Result<v1::ListArticlesResponse, Status>;
}

struct Articles<C>(C);

#[tonic::async_trait]
impl<C: Connector> ArticleService for Articles<C> {
    async fn list_articles(
        &self,
        request: Request<v1::ListArticlesRequest>,
    ) -> Result<Response<v1::ListArticlesResponse>, Status> {
        let workspace_id = workspace_of(request.metadata())?;
        let listed = self
            .0
            .list_articles(workspace_id, request.into_inner())
            .await?;

        Ok(Response::new(listed))
    }
}

fn workspace_of(metadata: &MetadataMap) -> Result<i64, Status> {
    metadata
        .get(WORKSPACE_METADATA)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| Status::invalid_argument("The workspace-id metadata must be a number"))
}

/// Answers the calls of the core on `listener` until `shutdown` ends.
pub async fn serve(
    listener: TcpListener,
    connector: impl Connector,
    shutdown: impl Future<Output = ()>,
) -> Result<(), Error> {
    let (health, health_service) = tonic_health::server::health_reporter();
    health
        .set_serving::<ArticleServiceServer<Articles<()>>>()
        .await;

    Server::builder()
        .add_service(health_service)
        .add_service(ArticleServiceServer::new(Articles(connector)))
        .serve_with_incoming_shutdown(TcpIncoming::from(listener), shutdown)
        .await
}

/// The core, as connect calls it: today only its health.
pub struct Core {
    health: HealthClient<Channel>,
}

impl Core {
    /// `url` is the gRPC port of the core, like `http://core:50051`.
    pub fn new(url: &str) -> Result<Self, Error> {
        let channel = Endpoint::new(url.to_owned())?.connect_lazy();

        Ok(Self {
            health: HealthClient::new(channel),
        })
    }

    /// Whether the core answers and says it is serving.
    pub async fn is_serving(&mut self) -> Result<bool, Status> {
        let checked = self.health.check(HealthCheckRequest::default()).await?;

        Ok(checked.into_inner().status() == ServingStatus::Serving)
    }
}
