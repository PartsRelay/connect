//! The link over real HTTP/2 sockets: the calls of the core, and the calls to it.

use std::sync::{Arc, Mutex};

use partsrelay_grpc::v1::article_service_client::ArticleServiceClient;
use partsrelay_grpc::{Connector, Core, WORKSPACE_METADATA, serve, v1};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tonic::transport::Endpoint;
use tonic::{Code, Request, Status};
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;

/// A management system with one article, that keeps who asked and what.
#[derive(Clone, Default)]
struct Shelf {
    asked: Arc<Mutex<Vec<(i64, v1::ListArticlesRequest)>>>,
}

fn pads() -> v1::Article {
    v1::Article {
        id: Some(v1::article::Id::IdNumber(1042)),
        brand: "BREMBO".into(),
        code: "P 23 141".into(),
        description: Some("Brake pad set".into()),
        quantity: 4,
        price: Some("48.90".into()),
        currency: Some("EUR".into()),
    }
}

#[tonic::async_trait]
impl Connector for Shelf {
    async fn list_articles(
        &self,
        workspace_id: i64,
        request: v1::ListArticlesRequest,
    ) -> Result<v1::ListArticlesResponse, Status> {
        self.asked.lock().unwrap().push((workspace_id, request));

        Ok(v1::ListArticlesResponse {
            articles: vec![pads()],
            total: 1,
        })
    }
}

/// Serves `shelf` on a free port and gives its address; it stops with the test.
async fn connect(shelf: Shelf) -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = oneshot::channel::<()>();

    tokio::spawn(serve(listener, shelf, async {
        stopped.await.ok();
    }));

    (url, stop)
}

#[tokio::test]
async fn lists_the_articles_of_the_company_in_the_metadata() {
    let shelf = Shelf::default();
    let (url, _stop) = connect(shelf.clone()).await;
    let asked = v1::ListArticlesRequest {
        codes: vec!["P 23 141".into()],
        ..Default::default()
    };
    let mut request = Request::new(asked.clone());
    request
        .metadata_mut()
        .insert(WORKSPACE_METADATA, "7".parse().unwrap());

    let listed = ArticleServiceClient::connect(url)
        .await
        .unwrap()
        .list_articles(request)
        .await;

    assert_eq!(
        listed.unwrap().into_inner(),
        v1::ListArticlesResponse {
            articles: vec![pads()],
            total: 1
        }
    );
    assert_eq!(*shelf.asked.lock().unwrap(), vec![(7, asked)]);
}

#[tokio::test]
async fn refuses_a_call_that_names_no_company() {
    let shelf = Shelf::default();
    let (url, _stop) = connect(shelf.clone()).await;
    let mut articles = ArticleServiceClient::connect(url).await.unwrap();
    let mut named_badly = Request::new(v1::ListArticlesRequest::default());
    named_badly
        .metadata_mut()
        .insert(WORKSPACE_METADATA, "seven".parse().unwrap());

    let unnamed = articles
        .list_articles(v1::ListArticlesRequest::default())
        .await
        .unwrap_err();
    let unreadable = articles.list_articles(named_badly).await.unwrap_err();

    assert_eq!(unnamed.code(), Code::InvalidArgument);
    assert_eq!(unreadable.code(), Code::InvalidArgument);
    assert!(shelf.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn answers_the_health_check_of_the_core() {
    let (url, _stop) = connect(Shelf::default()).await;
    let service = "partsrelay.connect_api.v1.ArticleService".to_owned();

    let channel = Endpoint::new(url).unwrap().connect().await.unwrap();

    let checked = HealthClient::new(channel)
        .check(HealthCheckRequest { service })
        .await
        .unwrap();

    assert_eq!(checked.into_inner().status(), ServingStatus::Serving);
}

#[tokio::test]
async fn checks_the_health_of_the_core() {
    // Any gRPC server with the health service stands in for the core
    let (url, _stop) = connect(Shelf::default()).await;
    let nowhere = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed = format!("http://{}", nowhere.local_addr().unwrap());
    drop(nowhere);

    assert!(Core::new(&url).unwrap().is_serving().await.unwrap());
    assert_eq!(
        Core::new(&closed)
            .unwrap()
            .is_serving()
            .await
            .unwrap_err()
            .code(),
        Code::Unavailable
    );
    assert!(Core::new("not an address").is_err());
}
