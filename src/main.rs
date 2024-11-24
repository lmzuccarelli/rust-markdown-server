use http::StatusCode;
use http_body_util::*;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{HeaderMap, Method};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn markdown(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let mut response = Response::new(Full::default());
    match req.method() {
        &Method::GET => {
            let uri = req.uri().path_and_query();
            let file = uri.unwrap().path().split("/").nth(1).unwrap();
            let data = fs::read_to_string(&format!("{}", file));
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "text/markdown".parse().unwrap());
            if data.is_err() {
                *response.body_mut() = Full::from("contents not found\n");
            } else {
                *response.headers_mut() = headers;
                *response.body_mut() = Full::from(data.unwrap());
            }
        }
        _ => {
            *response.status_mut() = StatusCode::NOT_FOUND;
        }
    };
    Ok(response)
}

// used for http2
#[derive(Clone)]
pub struct TokioExecutor;

impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn(fut);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(markdown))
                .await
            {
                eprintln!("Error serving connection: {}", err);
            }
        });
    }
}
