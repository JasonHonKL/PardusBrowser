use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{tungstenite, WebSocketStream};

use crate::domain::DomainContext;
use crate::protocol::event_bus::EventBus;
use crate::protocol::message::{CdpErrorResponse, CdpRequest};
use crate::protocol::router::CdpRouter;
use crate::protocol::target::CdpSession;

pub async fn handle_websocket(
    ws_stream: WebSocketStream<TcpStream>,
    router: Arc<CdpRouter>,
    ctx: Arc<DomainContext>,
    event_bus: Arc<EventBus>,
    timeout_secs: u64,
) {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut event_rx = event_bus.subscribe();
    let session = Arc::new(Mutex::new(CdpSession::new(
        uuid::Uuid::new_v4().to_string(),
    )));

    loop {
        tokio::select! {
            // Incoming CDP commands
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(text))) => {
                        handle_text_message(&text, &router, &ctx, &session, &mut ws_sender).await;
                    }
                    Some(Ok(tungstenite::Message::Ping(data))) => {
                        let _ = ws_sender.send(tungstenite::Message::Pong(data)).await;
                    }
                    Some(Ok(tungstenite::Message::Close(_))) | None => {
                        tracing::info!("WebSocket connection closed");
                        break;
                    }
                    Some(Ok(tungstenite::Message::Binary(data))) => {
                        // Try to parse as UTF-8 text.
                        if let Ok(text) = String::from_utf8(data.to_vec()) {
                            handle_text_message(&text, &router, &ctx, &session, &mut ws_sender).await;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            // Outgoing CDP events
            event = event_rx.recv() => {
                match event {
                    Ok(event) => {
                        let session = session.lock().await;
                        // Only send events for enabled domains.
                        let domain = event.method.split('.').next().unwrap_or("");
                        if session.is_domain_enabled(domain) || domain == "Target" {
                            let json = serde_json::to_string(&event).unwrap_or_default();
                            let _session_id = session.session_id.clone();
                            drop(session);
                            let msg = tungstenite::Message::Text(json.into());
                            if ws_sender.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Event bus lagged by {} messages", n);
                    }
                    Err(_) => break,
                }
            }
            // Inactivity timeout
            _ = tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)) => {
                tracing::info!("WebSocket connection timed out");
                break;
            }
        }
    }
}

async fn handle_text_message(
    text: &str,
    router: &Arc<CdpRouter>,
    ctx: &Arc<DomainContext>,
    session: &Arc<Mutex<CdpSession>>,
    ws_sender: &mut futures_util::stream::SplitSink<
        WebSocketStream<TcpStream>,
        tungstenite::Message,
    >,
) {
    let request: CdpRequest = match serde_json::from_str(text) {
        Ok(req) => req,
        Err(e) => {
            let err = CdpErrorResponse {
                id: 0,
                error: crate::error::CdpErrorBody {
                    code: crate::error::PARSE_ERROR,
                    message: format!("Parse error: {}", e),
                },
                session_id: None,
            };
            let json = serde_json::to_string(&err).unwrap_or_default();
            let _ = ws_sender.send(tungstenite::Message::Text(json.into())).await;
            return;
        }
    };

    let mut session = session.lock().await;
    match router.route(request, &mut session, ctx).await {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap_or_default();
            let _ = ws_sender.send(tungstenite::Message::Text(json.into())).await;
        }
        Err(error) => {
            let json = serde_json::to_string(&error).unwrap_or_default();
            let _ = ws_sender.send(tungstenite::Message::Text(json.into())).await;
        }
    }
}
