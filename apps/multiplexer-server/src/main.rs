//! Loopback JSON-RPC WebSocket server.

use std::net::SocketAddr;
use std::sync::Arc;

use multiplexer_server::{serve_listener, Server};

#[tokio::main]
async fn main() {
    let requested = listen_addr();
    let listener = tokio::net::TcpListener::bind(requested)
        .await
        .unwrap_or_else(|err| {
            eprintln!("bind {requested}: {err}");
            std::process::exit(1);
        });
    let addr = listener.local_addr().unwrap_or_else(|err| {
        eprintln!("local_addr: {err}");
        std::process::exit(1);
    });
    println!("listening on ws://{addr}");
    let server = Arc::new(Server::with_fake_provider());
    if let Err(err) = serve_listener(listener, server).await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn listen_addr() -> SocketAddr {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(8787);
    SocketAddr::from(([127, 0, 0, 1], port))
}
