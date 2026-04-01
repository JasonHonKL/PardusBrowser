use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::domain::browser::BrowserDomain;
use crate::domain::console::ConsoleDomain;
use crate::domain::css::CssDomain;
use crate::domain::dom::DomDomain;
use crate::domain::emulation::EmulationDomain;
use crate::domain::input::InputDomain;
use crate::domain::log::LogDomain;
use crate::domain::network::NetworkDomain;
use crate::domain::pardus_ext::PardusDomain;
use crate::domain::page::PageDomain;
use crate::domain::performance::PerformanceDomain;
use crate::domain::runtime::RuntimeDomain;
use crate::domain::security::SecurityDomain;
use crate::domain::target::TargetDomain;
use crate::domain::DomainContext;
use crate::protocol::event_bus::EventBus;
use crate::protocol::node_map::NodeMap;
use crate::protocol::registry::DomainRegistry;
use crate::protocol::router::CdpRouter;

/// CDP WebSocket server.
pub struct CdpServer {
    host: String,
    port: u16,
    timeout: u64,
    app: Arc<pardus_core::App>,
}

impl CdpServer {
    pub fn new(host: String, port: u16, timeout: u64, app: Arc<pardus_core::App>) -> Self {
        Self { host, port, timeout, app }
    }

    pub fn host(&self) -> &str { &self.host }
    pub fn port(&self) -> u16 { self.port }

    pub async fn run(self) -> anyhow::Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("CDP server listening on ws://{}", addr);
        tracing::info!("Discovery: http://{}/json/version", addr);

        let event_bus = Arc::new(EventBus::new(1024));
        let registry = build_registry();
        let router = Arc::new(CdpRouter::new(registry));

        loop {
            let (stream, _addr) = listener.accept().await?;
            let router = router.clone();
            let event_bus = event_bus.clone();
            let app = self.app.clone();
            let timeout = self.timeout;

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, router, event_bus, app, timeout).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    router: Arc<CdpRouter>,
    event_bus: Arc<EventBus>,
    app: Arc<pardus_core::App>,
    timeout: u64,
) -> anyhow::Result<()> {
    let ws_result = tokio_tungstenite::accept_async(stream).await;

    match ws_result {
        Ok(ws_stream) => {
            let targets = Arc::new(Mutex::new(HashMap::<String, crate::domain::TargetEntry>::new()));
            let node_map = Arc::new(Mutex::new(NodeMap::new()));
            let ctx = Arc::new(DomainContext {
                app,
                targets,
                event_tx: event_bus.sender(),
                node_map,
            });
            crate::transport::ws::handle_websocket(
                ws_stream, router, ctx, event_bus, timeout,
            ).await;
        }
        Err(e) => {
            tracing::debug!("Non-WebSocket connection (likely HTTP discovery): {}", e);
        }
    }

    Ok(())
}

fn build_registry() -> DomainRegistry {
    let mut registry = DomainRegistry::new();
    registry.register(Box::new(BrowserDomain));
    registry.register(Box::new(TargetDomain));
    registry.register(Box::new(PageDomain));
    registry.register(Box::new(RuntimeDomain));
    registry.register(Box::new(DomDomain));
    registry.register(Box::new(NetworkDomain));
    registry.register(Box::new(EmulationDomain));
    registry.register(Box::new(InputDomain));
    registry.register(Box::new(CssDomain));
    registry.register(Box::new(LogDomain));
    registry.register(Box::new(ConsoleDomain));
    registry.register(Box::new(SecurityDomain));
    registry.register(Box::new(PerformanceDomain));
    registry.register(Box::new(PardusDomain));
    registry
}
