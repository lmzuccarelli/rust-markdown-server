use http::StatusCode;
use http_body_util::*;
use hyper::body::Bytes;
use hyper::header::CONTENT_TYPE;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{HeaderMap, Method};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use surrealkv::{Tree, TreeBuilder};
use tokio::net::TcpListener;

async fn markdown(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let mut response = Response::new(Full::default());
    match req.method() {
        &Method::GET => {
            let req_uri = req.uri().to_string();
            let params: Vec<&str> = req_uri.split("/").collect();
            let key = params.last().unwrap().to_string();
            let result = db_read(key.clone()).await;
            match result {
                Ok(document) => {
                    fs::write(format!("./{}.md", key), document.clone())
                        .expect("should write file");
                    let mut headers = HeaderMap::new();
                    headers.insert(CONTENT_TYPE, "text/markdown".parse().unwrap());
                    *response.headers_mut() = headers;
                    *response.body_mut() = Full::from(document.to_string());
                }
                Err(e) => {
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    *response.body_mut() =
                        Full::from(format!("could no read db {}", e.to_string()));
                }
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

pub fn get_error(msg: String) -> Box<dyn std::error::Error> {
    Box::from(format!("{}", msg.to_lowercase()))
}

pub fn get_opts() -> Result<Tree, Box<dyn std::error::Error>> {
    let tree = TreeBuilder::new()
        .with_path(format!("{}.kv", "/home/lzuccarelli/database/documents").into())
        .with_max_memtable_size(100 * 1024 * 1024)
        .with_block_size(4096)
        .with_level_count(1);
    let t = tree.build()?;
    println!("[get_opts] tree built");
    Ok(t)
}

async fn db_read(key: String) -> Result<String, Box<dyn std::error::Error>> {
    let tree = get_opts()?;
    // start transaction
    let mut txn = tree.begin().map_err(|e| get_error(e.to_string()))?;
    let b_key = Bytes::from(key.clone());
    let res = txn.get(&b_key).map_err(|e| get_error(e.to_string()))?;
    // commit transaction
    txn.commit().await?;
    tree.close().await?;
    match res {
        Some(val) => {
            let document = String::from_utf8(val.to_vec())?;
            Ok(document)
        }
        None => {
            let msg = format!("no document found with key {}", key);
            println!("{}", msg);
            Err(get_error(msg))
        }
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
