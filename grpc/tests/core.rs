//! The client against a stand-in of the core that answers in gRPC-Web.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use bytes::{BufMut, Bytes, BytesMut};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use partsrelay_grpc::{Core, v1};
use prost::Message;
use tokio::net::TcpListener;
use tonic::Code;

const MESSAGE: u8 = 0;
const TRAILERS: u8 = 0x80;

/// What the stand-in was asked: path, authorization, content type and message.
type Asked = Arc<Mutex<Vec<(String, String, String, Bytes)>>>;

/// One gRPC-Web frame: its kind, its length and its bytes.
fn frame(kind: u8, payload: &[u8]) -> Bytes {
    let mut frame = BytesMut::with_capacity(5 + payload.len());
    frame.put_u8(kind);
    frame.put_u32(payload.len() as u32);
    frame.put_slice(payload);

    frame.freeze()
}

/// Answers every call with `response` and gives its address and its calls.
async fn core(response: Response<Full<Bytes>>) -> (String, Asked) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let asked = Asked::default();
    let calls = asked.clone();

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let (calls, response) = (calls.clone(), response.clone());
            let service = service_fn(move |request: Request<Incoming>| {
                let (calls, response) = (calls.clone(), response.clone());

                async move {
                    let header = |name: &str| {
                        let value = request.headers().get(name);
                        value.map_or_else(String::new, |value| value.to_str().unwrap().to_owned())
                    };
                    let call = (
                        request.uri().path().to_owned(),
                        header("authorization"),
                        header("content-type"),
                    );
                    let body = request.into_body().collect().await.unwrap().to_bytes();
                    calls.lock().unwrap().push((call.0, call.1, call.2, body));

                    Ok::<_, Infallible>(response)
                }
            });

            tokio::spawn(
                hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service),
            );
        }
    });

    (url, asked)
}

#[tokio::test]
async fn lists_the_articles_with_the_key_of_the_company() {
    let pads = v1::Article {
        brand: "BREMBO".into(),
        code: "P 23 141".into(),
        description: Some("Brake pad set".into()),
        quantity: 4,
        price: Some("48.90".into()),
        currency: Some("EUR".into()),
    };
    let listed = v1::ListArticlesResponse {
        articles: vec![pads],
        total: 1,
    };
    let body = [
        frame(MESSAGE, &listed.encode_to_vec()),
        frame(TRAILERS, b"grpc-status:0\r\n"),
    ]
    .concat();
    let answer = Response::builder()
        .header("content-type", "application/grpc-web+proto")
        .body(Full::new(Bytes::from(body)))
        .unwrap();
    let (url, asked) = core(answer).await;
    let request = v1::ListArticlesRequest {
        codes: vec!["P 23 141".into()],
        ..Default::default()
    };

    let articles = Core::new(&url, "test_key")
        .unwrap()
        .list_articles(request.clone())
        .await
        .unwrap();

    assert_eq!(articles, listed);

    let asked = asked.lock().unwrap();
    let (path, authorization, content_type, message) = &asked[0];
    assert_eq!(
        path,
        "/partsrelay.public_api.v1.ArticleService/ListArticles"
    );
    assert_eq!(authorization, "Bearer test_key");
    assert_eq!(content_type, "application/grpc-web");
    assert_eq!(message, &frame(MESSAGE, &request.encode_to_vec()));
}

#[tokio::test]
async fn gives_the_reason_of_a_refused_call() {
    let answer = Response::builder()
        .header("content-type", "application/grpc-web+proto")
        .header("grpc-status", "16")
        .header("grpc-message", "The API key is missing or not valid")
        .header("code", "invalid-api-key")
        .body(Full::default())
        .unwrap();
    let (url, _) = core(answer).await;

    let refused = Core::new(&url, "test_unknown")
        .unwrap()
        .list_articles(v1::ListArticlesRequest::default())
        .await
        .unwrap_err();

    assert_eq!(refused.code(), Code::Unauthenticated);
    assert_eq!(refused.message(), "The API key is missing or not valid");
    assert_eq!(refused.metadata().get("code").unwrap(), "invalid-api-key");
}

#[test]
fn refuses_an_address_or_a_key_it_cannot_send() {
    assert!(Core::new("not an address", "test_key").is_err());
    assert!(Core::new("https://core.example", "test_\nkey").is_err());
}
