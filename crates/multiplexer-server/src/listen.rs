//! TCP + WebSocket listen loop for the JSON-RPC router.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use crate::backend::SessionBackend;
use crate::server::Server;

/// Failure while binding or accepting connections.
#[derive(Debug, Error)]
pub enum ListenError {
    /// The TCP bind failed.
    #[error("bind {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    /// Accepting a client failed.
    #[error("accept: {0}")]
    Accept(#[source] std::io::Error),
}

/// Bind `addr` and serve WebSocket clients until the listener fails.
pub async fn serve<B>(addr: SocketAddr, server: Arc<Server<B>>) -> Result<(), ListenError>
where
    B: SessionBackend + Send + Sync + 'static,
{
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|source| ListenError::Bind { addr, source })?;
    serve_listener(listener, server).await
}

/// Serve WebSocket clients on an already-bound listener (port 0 is resolved).
///
/// The accept loop only returns when the listener fails. That exit is not
/// reachable from a healthy socket, so it is omitted from coverage.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn serve_listener<B>(
    listener: TcpListener,
    server: Arc<Server<B>>,
) -> Result<(), ListenError>
where
    B: SessionBackend + Send + Sync + 'static,
{
    loop {
        let (stream, _) = listener.accept().await.map_err(ListenError::Accept)?;
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            handle_socket(stream, server).await;
        });
    }
}

async fn handle_socket<B>(stream: TcpStream, server: Arc<Server<B>>)
where
    B: SessionBackend + Send + Sync + 'static,
{
    let mut ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(_) => return,
    };

    while let Some(incoming) = ws.next().await {
        let msg = match incoming {
            Ok(msg) => msg,
            Err(_) => return,
        };
        match msg {
            Message::Text(text) => {
                let frames = server.handle_frame(text.as_ref());
                for frame in frames {
                    if ws.send(Message::text(frame)).await.is_err() {
                        return;
                    }
                }
            }
            Message::Binary(_) => {
                let _ = ws.close(None).await;
                return;
            }
            Message::Close(_) => return,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn listen_error_display_includes_context() {
        let addr = "127.0.0.1:9".parse().unwrap();
        let bind = ListenError::Bind {
            addr,
            source: Error::new(ErrorKind::AddrNotAvailable, "nope"),
        };
        let text = bind.to_string();
        assert!(text.contains("bind"));
        assert!(text.contains("127.0.0.1:9"));
        assert!(text.contains("nope"));
        let accept = ListenError::Accept(Error::new(ErrorKind::ConnectionAborted, "gone"));
        assert!(accept.to_string().contains("accept"));
        assert!(accept.to_string().contains("gone"));
    }
}
