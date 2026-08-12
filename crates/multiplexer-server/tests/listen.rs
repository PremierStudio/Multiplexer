//! WebSocket listen loop. Bind an ephemeral port, speak session.start, drop.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use multiplexer_server::{serve, serve_listener, ListenError, Server};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[tokio::test(flavor = "multi_thread")]
async fn session_start_over_websocket() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("local addr");
        let server = Arc::new(Server::new());
        tokio::spawn(async move {
            let _ = serve_listener(listener, server).await;
        });

        let url = format!("ws://{addr}");
        let (mut client, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("connect websocket");

        let request = encode_frame(&Message::Request(Request::new(
            Id::String("s1".into()),
            methods::SESSION_START,
            json!({
                "provider": "grok",
                "model": "grok-4",
                "workspace": "/ws",
            }),
        )))
        .expect("request encodes");
        client
            .send(WsMessage::text(request))
            .await
            .expect("send session.start");

        let mut session_id = None;
        while let Some(incoming) = client.next().await {
            let WsMessage::Text(text) = incoming.expect("ws frame") else {
                continue;
            };
            let text: &str = text.as_ref();
            if text.is_empty() {
                continue;
            }
            match decode_frame(text) {
                Ok(Message::Response(resp)) => {
                    session_id = resp
                        .result
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                    break;
                }
                Ok(_) | Err(_) => continue,
            }
        }
        assert!(
            session_id.as_deref().is_some_and(|id| !id.is_empty()),
            "session_id present in response"
        );
        drop(client);
    })
    .await
    .expect("listen test timed out");
}

#[tokio::test]
async fn serve_returns_bind_error_for_non_local_addr() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let addr = "8.8.8.8:8787".parse().expect("addr");
        let server = Arc::new(Server::new());
        let err = serve(addr, server).await;
        assert!(
            matches!(err, Err(ListenError::Bind { addr: got, .. }) if got == addr),
            "expected bind error, got {err:?}"
        );
        assert!(err.unwrap_err().to_string().contains("bind"));
    })
    .await
    .expect("bind-error test timed out");
}

#[tokio::test]
async fn serve_binds_ephemeral_port() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let addr = "127.0.0.1:0".parse().expect("addr");
        let server = Arc::new(Server::new());
        let handle = tokio::spawn(serve(addr, server));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!handle.is_finished());
        handle.abort();
        let _ = handle.await;
    })
    .await
    .expect("serve bind test timed out");
}

#[tokio::test]
async fn binary_close_ping_and_bad_handshake_are_ignored_or_dropped() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("local addr");
        let server = Arc::new(Server::new());
        tokio::spawn(async move {
            let _ = serve_listener(listener, server).await;
        });

        let url = format!("ws://{addr}");
        let (mut client, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("connect websocket");
        client
            .send(WsMessage::Ping(vec![1, 2, 3].into()))
            .await
            .expect("ping");
        client
            .send(WsMessage::Pong(vec![].into()))
            .await
            .expect("pong");
        client
            .send(WsMessage::Binary(vec![9, 9].into()))
            .await
            .expect("binary");
        let _ = client.next().await;

        let (mut client2, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("connect again");
        client2.send(WsMessage::Close(None)).await.expect("close");
        drop(client2);

        let request = encode_frame(&Message::Request(Request::new(
            Id::String("s1".into()),
            methods::SESSION_START,
            json!({
                "provider": "grok",
                "model": "grok-4",
                "workspace": "/ws",
            }),
        )))
        .expect("request encodes");
        let (mut client3, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("connect third");
        client3.send(WsMessage::text(request)).await.expect("send");
        drop(client3);

        let mut raw = tokio::net::TcpStream::connect(addr).await.expect("tcp");
        use tokio::io::AsyncWriteExt;
        raw.write_all(b"GET / HTTP/1.1\r\n\r\n")
            .await
            .expect("write garbage");
        drop(raw);
        tokio::time::sleep(Duration::from_millis(50)).await;
    })
    .await
    .expect("socket-path test timed out");
}
