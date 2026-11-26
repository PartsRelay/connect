//! Client of the Parts Relay public API, generated from its proto files.
//!
//! The core answers the API on its HTTPS port as gRPC-Web, so a call is a
//! plain HTTP request that passes any proxy.

use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::{Request, Status};
use tonic_web::{GrpcWebCall, GrpcWebClientLayer, GrpcWebClientService};
use tower::Layer;

/// The messages and the client of `partsrelay.public_api.v1`.
pub mod v1 {
    tonic::include_proto!("partsrelay.public_api.v1");
}

use v1::article_service_client::ArticleServiceClient;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

type Body = GrpcWebCall<tonic::body::Body>;
type Http = GrpcWebClientService<Client<HttpsConnector<HttpConnector>, Body>>;

/// The public API of a core, called with the API key of a company.
#[derive(Clone)]
pub struct Core {
    articles: ArticleServiceClient<Http>,
    authorization: MetadataValue<Ascii>,
}

impl Core {
    /// `url` is the address of the core, `api_key` a `test_` or a `live_` key.
    pub fn new(url: &str, api_key: &str) -> Result<Self, Error> {
        let origin: http::Uri = url.parse()?;
        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();
        let http =
            GrpcWebClientLayer::new().layer(Client::builder(TokioExecutor::new()).build(connector));

        Ok(Self {
            articles: ArticleServiceClient::with_origin(http, origin),
            authorization: format!("Bearer {api_key}").parse()?,
        })
    }

    pub async fn list_articles(
        &mut self,
        request: v1::ListArticlesRequest,
    ) -> Result<v1::ListArticlesResponse, Status> {
        let request = self.authorized(request);

        Ok(self.articles.list_articles(request).await?.into_inner())
    }

    fn authorized<T>(&self, message: T) -> Request<T> {
        let mut request = Request::new(message);
        request
            .metadata_mut()
            .insert("authorization", self.authorization.clone());

        request
    }
}
